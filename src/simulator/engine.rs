use crate::cli::RunOpts;
use crate::hardware::{
    csr::CsrFile,
    fp_registers::FpRegisters,
    memory::{Memory, STACK_TOP},
    registers::RegisterFile,
};
use crate::isa::{
    formats as f, fp,
    rv32i::{step as rv32i_step, StepResult},
    rv32m,
};
use crate::simulator::syscalls::{self, GuiSyscallOutcome};
use anyhow::Result;
use serde::Serialize;
use std::collections::VecDeque;
use std::io::{BufRead, Write};

pub struct CpuState {
    pub regs: RegisterFile,
    pub fp: FpRegisters,
    pub csr: CsrFile,
    pub mem: Memory,
    pub pc: u32,
    pub instret: u64,
}

impl CpuState {
    pub fn new(entry: u32) -> Self {
        let mut regs = RegisterFile::new();
        regs.write(2, STACK_TOP); // sp
        Self {
            regs,
            fp: FpRegisters::new(),
            csr: CsrFile::new(),
            mem: Memory::new(),
            pc: entry,
            instret: 0,
        }
    }
}

#[derive(Serialize)]
pub struct Telemetry {
    pub instructions: u64,
    pub exit_code: i32,
}

// ─── Single-step interface (for the GUI) ─────────────────────────────────────

pub enum StepOutcome {
    Continue,
    NeedInput,
    Halted(i32),
    Faulted(String),
}

/// Execute one instruction.  Console output is appended to `console`;
/// stdin is drawn from `input_queue` (returns NeedInput if the queue is empty
/// when a read syscall fires).
pub fn step_one(
    cpu: &mut CpuState,
    console: &mut String,
    input_queue: &mut VecDeque<String>,
) -> StepOutcome {
    let word = cpu.mem.load_word(cpu.pc);
    let opc = f::opcode(word);

    let raw = match opc {
        0x07 | 0x27 | 0x43 | 0x47 | 0x4B | 0x4F | 0x53 => {
            fp::step(word, cpu.pc, &mut cpu.regs, &mut cpu.fp, &mut cpu.mem)
                .map(|next| (next, false, false))
        }

        0x33 if f::funct7(word) == 0x01 => {
            rv32m::step(word, cpu.pc, &mut cpu.regs).map(|next| (next, false, false))
        }

        0x73 if f::funct3(word) != 0 => exec_csr(word, cpu.pc, &mut cpu.regs, &mut cpu.csr)
            .map(|()| (cpu.pc.wrapping_add(4), false, false)),

        _ => rv32i_step(word, cpu.pc, &mut cpu.regs, &mut cpu.mem).map(|sr| match sr {
            StepResult::Next(pc) => (pc, false, false),
            StepResult::Ecall => (0, true, false),
            StepResult::Ebreak => (0, false, true),
        }),
    };

    match raw {
        Err(e) => StepOutcome::Faulted(e.to_string()),

        Ok((_, _, true)) => StepOutcome::Halted(0), // ebreak

        Ok((_, true, _)) => {
            // ecall — dispatch through GUI path
            match syscalls::dispatch_gui(
                &mut cpu.regs,
                &mut cpu.fp,
                &mut cpu.mem,
                cpu.pc,
                console,
                input_queue,
            ) {
                Err(e) => StepOutcome::Faulted(e.to_string()),
                Ok(GuiSyscallOutcome::NeedInput) => StepOutcome::NeedInput,
                Ok(GuiSyscallOutcome::Halt) => StepOutcome::Halted(cpu.regs.read(10) as i32),
                Ok(GuiSyscallOutcome::Continue) => {
                    cpu.pc = cpu.pc.wrapping_add(4);
                    cpu.instret += 1;
                    cpu.csr.tick(cpu.instret);
                    StepOutcome::Continue
                }
            }
        }

        Ok((next_pc, false, false)) => {
            cpu.pc = next_pc;
            cpu.instret += 1;
            cpu.csr.tick(cpu.instret);
            StepOutcome::Continue
        }
    }
}

// ─── Headless batch runner ────────────────────────────────────────────────────

pub fn run(
    cpu: &mut CpuState,
    opts: &RunOpts,
    stdout: &mut dyn Write,
    stdin: &mut dyn BufRead,
) -> Result<Telemetry> {
    let mut steps: u64 = 0;

    loop {
        if opts.max_steps > 0 && steps >= opts.max_steps {
            break;
        }

        let word = cpu.mem.load_word(cpu.pc);
        let opc = f::opcode(word);

        let next_pc = match opc {
            // ── FP instructions ───────────────────────────────────────────────
            0x07 | 0x27 | 0x43 | 0x47 | 0x4B | 0x4F | 0x53 => {
                fp::step(word, cpu.pc, &mut cpu.regs, &mut cpu.fp, &mut cpu.mem)?
            }

            // ── RV32M (MUL/DIV family): opcode 0x33 with funct7=0x01 ──────────
            0x33 if f::funct7(word) == 0x01 => rv32m::step(word, cpu.pc, &mut cpu.regs)?,

            // ── CSR instructions: opcode 0x73, funct3 != 0 ───────────────────
            0x73 if f::funct3(word) != 0 => {
                exec_csr(word, cpu.pc, &mut cpu.regs, &mut cpu.csr)?;
                cpu.pc.wrapping_add(4)
            }

            // ── Base RV32I (ecall/ebreak also land here via 0x73 funct3=0) ────
            _ => match rv32i_step(word, cpu.pc, &mut cpu.regs, &mut cpu.mem)? {
                StepResult::Next(pc) => pc,
                StepResult::Ecall => {
                    let pc = cpu.pc;
                    let cont = syscalls::dispatch(
                        &mut cpu.regs,
                        &mut cpu.fp,
                        &mut cpu.mem,
                        pc,
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
                    cpu.pc.wrapping_add(4)
                }
                StepResult::Ebreak => break,
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

fn exec_csr(
    word: u32,
    pc: u32,
    regs: &mut RegisterFile,
    csr: &mut CsrFile,
) -> Result<(), crate::util::error::OarsError> {
    let f3 = f::funct3(word);
    let rd = f::rd(word);
    let rs1 = f::rs1(word);
    let csr_addr = word >> 20; // bits 31:20
    let uimm = rs1 as u32; // for csrrwi/csrrsi/csrrci, rs1 field = uimm

    match f3 {
        0x1 => {
            // CSRRW
            let old = csr.read(csr_addr);
            if rd != 0 {
                regs.write(rd, old);
            }
            csr.write(csr_addr, regs.read(rs1));
        }
        0x2 => {
            // CSRRS
            let old = csr.read(csr_addr);
            regs.write(rd, old);
            if rs1 != 0 {
                csr.set_bits(csr_addr, regs.read(rs1));
            }
        }
        0x3 => {
            // CSRRC
            let old = csr.read(csr_addr);
            regs.write(rd, old);
            if rs1 != 0 {
                csr.clear_bits(csr_addr, regs.read(rs1));
            }
        }
        0x5 => {
            // CSRRWI
            let old = csr.read(csr_addr);
            if rd != 0 {
                regs.write(rd, old);
            }
            csr.write(csr_addr, uimm);
        }
        0x6 => {
            // CSRRSI
            let old = csr.read(csr_addr);
            regs.write(rd, old);
            if uimm != 0 {
                csr.set_bits(csr_addr, uimm);
            }
        }
        0x7 => {
            // CSRRCI
            let old = csr.read(csr_addr);
            regs.write(rd, old);
            if uimm != 0 {
                csr.clear_bits(csr_addr, uimm);
            }
        }
        _ => {
            return Err(crate::util::error::OarsError::Runtime {
                pc,
                msg: format!("unknown CSR funct3={f3:#x} at {pc:#010x}"),
            })
        }
    }

    Ok(())
}
