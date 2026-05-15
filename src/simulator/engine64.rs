use crate::cli::RunOpts;
use crate::hardware::{
    csr::CsrFile,
    fp_registers::FpRegisters,
    memory::{Memory, STACK_TOP},
    registers::RegisterFile,
    registers64::RegFile64,
};
use crate::isa::formats as f;
use crate::simulator::{
    engine::{StepOutcome, Telemetry},
    syscalls::{self, GuiSyscallOutcome},
};
use crate::util::error::OarsError;
use anyhow::Result;
use std::collections::VecDeque;
use std::io::{BufRead, Write};

pub struct CpuState64 {
    pub regs: RegFile64,
    pub fp: FpRegisters,
    pub csr: CsrFile,
    pub mem: Memory,
    pub pc: u64,
    pub instret: u64,
}

impl CpuState64 {
    pub fn new(entry: u32) -> Self {
        let mut regs = RegFile64::new();
        regs.write(2, STACK_TOP as u64); // sp
        Self {
            regs,
            fp: FpRegisters::new(),
            csr: CsrFile::new(),
            mem: Memory::new(),
            pc: entry as u64,
            instret: 0,
        }
    }
}

// ─── Single-step interface ────────────────────────────────────────────────────

pub fn step_one64(
    cpu: &mut CpuState64,
    console: &mut String,
    input_queue: &mut VecDeque<String>,
) -> StepOutcome {
    let pc32 = cpu.pc as u32;
    let word = cpu.mem.load_word(pc32);
    let opc = f::opcode(word);

    enum R {
        Next(u64),
        Ecall,
        Ebreak,
        Fault(String),
    }

    let sr = match opc {
        // ── FP (adapter through temporary RegisterFile) ───────────────────────
        0x07 | 0x27 | 0x43 | 0x47 | 0x4B | 0x4F | 0x53 => {
            match fp_step_64(word, pc32, &mut cpu.regs, &mut cpu.fp, &mut cpu.mem) {
                Ok(next) => R::Next(next as u64),
                Err(e) => R::Fault(e.to_string()),
            }
        }

        // ── RV64M: MUL/DIV/REM (64-bit wide) ─────────────────────────────────
        0x33 if f::funct7(word) == 0x01 => match rv64m_step(word, pc32, &mut cpu.regs) {
            Ok(next) => R::Next(next as u64),
            Err(e) => R::Fault(e.to_string()),
        },

        // ── CSR ───────────────────────────────────────────────────────────────
        0x73 if f::funct3(word) != 0 => match exec_csr64(word, pc32, &mut cpu.regs, &mut cpu.csr) {
            Ok(()) => R::Next(cpu.pc + 4),
            Err(e) => R::Fault(e.to_string()),
        },

        // ── RV64I base + W-suffix ─────────────────────────────────────────────
        _ => match rv64i_step(word, cpu.pc, &mut cpu.regs, &mut cpu.mem) {
            Err(e) => R::Fault(e.to_string()),
            Ok(Sr::Next(pc)) => R::Next(pc),
            Ok(Sr::Ecall) => R::Ecall,
            Ok(Sr::Ebreak) => R::Ebreak,
        },
    };

    match sr {
        R::Fault(e) => StepOutcome::Faulted(e),
        R::Ebreak => StepOutcome::Halted(0),
        R::Ecall => {
            match syscall_gui_64(
                &mut cpu.regs,
                &mut cpu.fp,
                &mut cpu.mem,
                pc32,
                console,
                input_queue,
            ) {
                Err(e) => StepOutcome::Faulted(e.to_string()),
                Ok(GuiSyscallOutcome::NeedInput) => StepOutcome::NeedInput,
                Ok(GuiSyscallOutcome::NeedChar) => StepOutcome::NeedChar,
                Ok(GuiSyscallOutcome::Halt) => StepOutcome::Halted(cpu.regs.read(10) as i64 as i32),
                Ok(GuiSyscallOutcome::Continue) => {
                    cpu.pc += 4;
                    cpu.instret += 1;
                    cpu.csr.tick(cpu.instret);
                    StepOutcome::Continue
                }
            }
        }
        R::Next(next_pc) => {
            cpu.pc = next_pc;
            cpu.instret += 1;
            cpu.csr.tick(cpu.instret);
            StepOutcome::Continue
        }
    }
}

// ─── Headless batch runner ────────────────────────────────────────────────────

pub fn run64(
    cpu: &mut CpuState64,
    opts: &RunOpts,
    stdout: &mut dyn Write,
    stdin: &mut dyn BufRead,
) -> Result<Telemetry> {
    let mut steps: u64 = 0;

    loop {
        if opts.max_steps > 0 && steps >= opts.max_steps {
            break;
        }

        let pc32 = cpu.pc as u32;
        let word = cpu.mem.load_word(pc32);
        let opc = f::opcode(word);

        let next_pc: u64 = match opc {
            0x07 | 0x27 | 0x43 | 0x47 | 0x4B | 0x4F | 0x53 => {
                fp_step_64(word, pc32, &mut cpu.regs, &mut cpu.fp, &mut cpu.mem)? as u64
            }

            0x33 if f::funct7(word) == 0x01 => rv64m_step(word, pc32, &mut cpu.regs)? as u64,

            0x73 if f::funct3(word) != 0 => {
                exec_csr64(word, pc32, &mut cpu.regs, &mut cpu.csr)?;
                cpu.pc + 4
            }

            _ => match rv64i_step(word, cpu.pc, &mut cpu.regs, &mut cpu.mem)? {
                Sr::Next(pc) => pc,
                Sr::Ecall => {
                    let pc = cpu.pc;
                    let cont = syscalls::dispatch(
                        &mut adapt_regs_for_dispatch(&cpu.regs),
                        &mut cpu.fp,
                        &mut cpu.mem,
                        pc as u32,
                        stdout,
                        stdin,
                    )?;
                    if !cont {
                        let exit_code = cpu.regs.read(10) as i32;
                        return Ok(Telemetry {
                            instructions: steps,
                            exit_code,
                        });
                    }
                    cpu.pc + 4
                }
                Sr::Ebreak => break,
            },
        };

        cpu.pc = next_pc;
        steps += 1;
        cpu.instret += 1;
        cpu.csr.tick(cpu.instret);
    }

    Ok(Telemetry {
        instructions: steps,
        exit_code: 0,
    })
}

// ─── RV64I instruction step ───────────────────────────────────────────────────

enum Sr {
    Next(u64),
    Ecall,
    Ebreak,
}

fn rv64i_step(word: u32, pc: u64, regs: &mut RegFile64, mem: &mut Memory) -> Result<Sr, OarsError> {
    let pc32 = pc as u32;

    match f::opcode(word) {
        // ── R-type (64-bit arithmetic) ────────────────────────────────────────
        0x33 => {
            let a = regs.read(f::rs1(word));
            let b = regs.read(f::rs2(word));
            let shamt = (b & 0x3F) as u32;
            let v: u64 = match (f::funct3(word), f::funct7(word)) {
                (0x0, 0x00) => a.wrapping_add(b),
                (0x0, 0x20) => a.wrapping_sub(b),
                (0x1, _) => a << shamt,
                (0x2, _) => ((a as i64) < (b as i64)) as u64,
                (0x3, _) => (a < b) as u64,
                (0x4, _) => a ^ b,
                (0x5, 0x00) => a >> shamt,
                (0x5, 0x20) => ((a as i64) >> shamt) as u64,
                (0x6, _) => a | b,
                (0x7, _) => a & b,
                _ => return Err(illegal(pc32, word)),
            };
            regs.write(f::rd(word), v);
            Ok(Sr::Next(pc + 4))
        }

        // ── I-type arithmetic (64-bit) ────────────────────────────────────────
        0x13 => {
            let a = regs.read(f::rs1(word));
            let imm_i = f::imm_i(word);
            let simm64 = imm_i as i64 as u64;
            let shamt = (word >> 20) & 0x3F;
            let v: u64 = match f::funct3(word) {
                0x0 => a.wrapping_add(simm64),
                0x2 => ((a as i64) < imm_i as i64) as u64,
                0x3 => (a < simm64) as u64,
                0x4 => a ^ simm64,
                0x6 => a | simm64,
                0x7 => a & simm64,
                0x1 => a << shamt,
                0x5 => {
                    if (word >> 30) & 1 == 1 {
                        ((a as i64) >> shamt) as u64
                    } else {
                        a >> shamt
                    }
                }
                _ => return Err(illegal(pc32, word)),
            };
            regs.write(f::rd(word), v);
            Ok(Sr::Next(pc + 4))
        }

        // ── OP-IMM-32: ADDIW, SLLIW, SRLIW, SRAIW ────────────────────────────
        0x1B => {
            let a = regs.read(f::rs1(word)) as u32;
            let imm_i = f::imm_i(word);
            let shamt5 = (word >> 20) & 0x1F;
            let v32: u32 = match f::funct3(word) {
                0x0 => a.wrapping_add(imm_i as u32),
                0x1 => a << shamt5,
                0x5 => {
                    if f::funct7(word) == 0x20 {
                        ((a as i32) >> shamt5) as u32
                    } else {
                        a >> shamt5
                    }
                }
                _ => return Err(illegal(pc32, word)),
            };
            regs.write(f::rd(word), v32 as i32 as i64 as u64);
            Ok(Sr::Next(pc + 4))
        }

        // ── OP-32: ADDW, SUBW, SLLW, SRLW, SRAW ─────────────────────────────
        0x3B => {
            let a = regs.read(f::rs1(word)) as u32;
            let b = regs.read(f::rs2(word)) as u32;
            let shamt5 = b & 0x1F;
            let v32: u32 = match (f::funct3(word), f::funct7(word)) {
                (0x0, 0x00) => a.wrapping_add(b),
                (0x0, 0x20) => a.wrapping_sub(b),
                (0x1, 0x00) => a << shamt5,
                (0x5, 0x00) => a >> shamt5,
                (0x5, 0x20) => ((a as i32) >> shamt5) as u32,
                _ => return Err(illegal(pc32, word)),
            };
            regs.write(f::rd(word), v32 as i32 as i64 as u64);
            Ok(Sr::Next(pc + 4))
        }

        // ── Load ──────────────────────────────────────────────────────────────
        0x03 => {
            let base = regs.read(f::rs1(word));
            let addr = (base as i64).wrapping_add(f::imm_i(word) as i64) as u64 as u32;
            let v: u64 = match f::funct3(word) {
                0x0 => mem.load_byte(addr) as i8 as i64 as u64,
                0x1 => mem.load_halfword(addr) as i16 as i64 as u64,
                0x2 => mem.load_word(addr) as i32 as i64 as u64,
                0x3 => mem.load_doubleword(addr),
                0x4 => mem.load_byte(addr) as u64,
                0x5 => mem.load_halfword(addr) as u64,
                0x6 => mem.load_word(addr) as u64,
                _ => return Err(illegal(pc32, word)),
            };
            regs.write(f::rd(word), v);
            Ok(Sr::Next(pc + 4))
        }

        // ── Store ─────────────────────────────────────────────────────────────
        0x23 => {
            let base = regs.read(f::rs1(word));
            let addr = (base as i64).wrapping_add(f::imm_s(word) as i64) as u64 as u32;
            let val = regs.read(f::rs2(word));
            match f::funct3(word) {
                0x0 => mem.store_byte(addr, val as u8),
                0x1 => mem.store_halfword(addr, val as u16),
                0x2 => mem.store_word(addr, val as u32),
                0x3 => mem.store_doubleword(addr, val),
                _ => return Err(illegal(pc32, word)),
            }
            Ok(Sr::Next(pc + 4))
        }

        // ── Branch ────────────────────────────────────────────────────────────
        0x63 => {
            let a = regs.read(f::rs1(word));
            let b = regs.read(f::rs2(word));
            let taken = match f::funct3(word) {
                0x0 => a == b,
                0x1 => a != b,
                0x4 => (a as i64) < (b as i64),
                0x5 => (a as i64) >= (b as i64),
                0x6 => a < b,
                0x7 => a >= b,
                _ => return Err(illegal(pc32, word)),
            };
            let next = if taken {
                (pc as i64).wrapping_add(f::imm_b(word) as i64) as u64
            } else {
                pc + 4
            };
            Ok(Sr::Next(next))
        }

        // ── JAL ───────────────────────────────────────────────────────────────
        0x6F => {
            let target = (pc as i64).wrapping_add(f::imm_j(word) as i64) as u64;
            regs.write(f::rd(word), pc + 4);
            Ok(Sr::Next(target))
        }

        // ── JALR ──────────────────────────────────────────────────────────────
        0x67 => {
            let base = regs.read(f::rs1(word));
            let target = (base as i64).wrapping_add(f::imm_i(word) as i64) as u64 & !1u64;
            regs.write(f::rd(word), pc + 4);
            Ok(Sr::Next(target))
        }

        // ── LUI ───────────────────────────────────────────────────────────────
        0x37 => {
            regs.write(f::rd(word), f::imm_u(word) as i32 as i64 as u64);
            Ok(Sr::Next(pc + 4))
        }

        // ── AUIPC ─────────────────────────────────────────────────────────────
        0x17 => {
            let imm = f::imm_u(word) as i32 as i64 as u64;
            regs.write(f::rd(word), pc.wrapping_add(imm));
            Ok(Sr::Next(pc + 4))
        }

        // ── FENCE: NOP ────────────────────────────────────────────────────────
        0x0F => Ok(Sr::Next(pc + 4)),

        // ── SYSTEM ───────────────────────────────────────────────────────────
        0x73 => match f::imm_i(word) as u32 {
            0x000 => Ok(Sr::Ecall),
            0x001 => Ok(Sr::Ebreak),
            _ => Err(illegal(pc32, word)),
        },

        _ => Err(illegal(pc32, word)),
    }
}

// ─── RV64M: 64-bit MUL/DIV/REM ───────────────────────────────────────────────

fn rv64m_step(word: u32, pc: u32, regs: &mut RegFile64) -> Result<u32, OarsError> {
    let a = regs.read(f::rs1(word));
    let b = regs.read(f::rs2(word));
    let v: u64 = match f::funct3(word) {
        0x0 => a.wrapping_mul(b),
        0x1 => {
            let r = (a as i64 as i128).wrapping_mul(b as i64 as i128);
            (r >> 64) as u64
        }
        0x2 => {
            let r = (a as i64 as i128).wrapping_mul(b as u128 as i128);
            (r >> 64) as u64
        }
        0x3 => {
            let r = (a as u128).wrapping_mul(b as u128);
            (r >> 64) as u64
        }
        0x4 => {
            if b == 0 {
                u64::MAX
            } else if a as i64 == i64::MIN && b as i64 == -1 {
                a
            } else {
                ((a as i64).wrapping_div(b as i64)) as u64
            }
        }
        0x5 => {
            if b == 0 {
                u64::MAX
            } else {
                a.wrapping_div(b)
            }
        }
        0x6 => {
            if b == 0 {
                a
            } else if a as i64 == i64::MIN && b as i64 == -1 {
                0
            } else {
                ((a as i64).wrapping_rem(b as i64)) as u64
            }
        }
        0x7 => {
            if b == 0 {
                a
            } else {
                a.wrapping_rem(b)
            }
        }
        _ => return Err(illegal(pc, word)),
    };
    regs.write(f::rd(word), v);
    Ok(pc.wrapping_add(4))
}

// ─── CSR (64-bit register file) ───────────────────────────────────────────────

fn exec_csr64(
    word: u32,
    pc: u32,
    regs: &mut RegFile64,
    csr: &mut CsrFile,
) -> Result<(), OarsError> {
    let f3 = f::funct3(word);
    let rd = f::rd(word);
    let rs1 = f::rs1(word);
    let csr_addr = word >> 20;
    let uimm = rs1 as u32;

    match f3 {
        0x1 => {
            let old = csr.read(csr_addr);
            if rd != 0 {
                regs.write(rd, old as u64);
            }
            csr.write(csr_addr, regs.read(rs1) as u32);
        }
        0x2 => {
            let old = csr.read(csr_addr);
            regs.write(rd, old as u64);
            if rs1 != 0 {
                csr.set_bits(csr_addr, regs.read(rs1) as u32);
            }
        }
        0x3 => {
            let old = csr.read(csr_addr);
            regs.write(rd, old as u64);
            if rs1 != 0 {
                csr.clear_bits(csr_addr, regs.read(rs1) as u32);
            }
        }
        0x5 => {
            let old = csr.read(csr_addr);
            if rd != 0 {
                regs.write(rd, old as u64);
            }
            csr.write(csr_addr, uimm);
        }
        0x6 => {
            let old = csr.read(csr_addr);
            regs.write(rd, old as u64);
            if uimm != 0 {
                csr.set_bits(csr_addr, uimm);
            }
        }
        0x7 => {
            let old = csr.read(csr_addr);
            regs.write(rd, old as u64);
            if uimm != 0 {
                csr.clear_bits(csr_addr, uimm);
            }
        }
        _ => {
            return Err(OarsError::Runtime {
                pc,
                msg: format!("unknown CSR funct3={f3:#x} at {pc:#010x}"),
            })
        }
    }
    Ok(())
}

// ─── FP adapter (bridges RegFile64 → RegisterFile for existing fp::step) ──────

fn fp_step_64(
    word: u32,
    pc: u32,
    regs64: &mut RegFile64,
    fp: &mut FpRegisters,
    mem: &mut Memory,
) -> Result<u32, OarsError> {
    let mut tmp = RegisterFile::new();
    for i in 0..32 {
        tmp.write(i, regs64.read(i) as u32);
    }
    let next = crate::isa::fp::step(word, pc, &mut tmp, fp, mem)?;
    let rd = f::rd(word);
    regs64.write(rd, tmp.read(rd) as i32 as i64 as u64);
    Ok(next)
}

// ─── Syscall adapter ─────────────────────────────────────────────────────────

fn syscall_gui_64(
    regs64: &mut RegFile64,
    fp: &mut FpRegisters,
    mem: &mut Memory,
    pc: u32,
    console: &mut String,
    input_queue: &mut VecDeque<String>,
) -> Result<GuiSyscallOutcome, OarsError> {
    let mut tmp = adapt_regs_for_dispatch(regs64);
    let result = syscalls::dispatch_gui(&mut tmp, fp, mem, pc, console, input_queue);
    regs64.write(10, tmp.read(10) as i32 as i64 as u64);
    result
}

fn adapt_regs_for_dispatch(regs64: &RegFile64) -> RegisterFile {
    let mut tmp = RegisterFile::new();
    for i in 0..32 {
        tmp.write(i, regs64.read(i) as u32);
    }
    tmp
}

fn illegal(pc: u32, word: u32) -> OarsError {
    OarsError::Runtime {
        pc,
        msg: format!("illegal instruction {word:#010x}"),
    }
}
