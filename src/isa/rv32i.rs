use crate::hardware::{memory::Memory, registers::RegisterFile};
use crate::isa::formats as f;
use crate::util::error::OarsError;

/// Result of executing one instruction.
pub enum StepResult {
    /// Normal execution; value is the next PC.
    Next(u32),
    /// ECALL instruction — engine reads a7 and dispatches.
    Ecall,
    /// EBREAK — break to debugger / halt in CLI mode.
    Ebreak,
}

/// Execute one 32-bit instruction word. Returns a `StepResult`.
pub fn step(
    word: u32,
    pc: u32,
    regs: &mut RegisterFile,
    mem: &mut Memory,
) -> Result<StepResult, OarsError> {
    match f::opcode(word) {
        // ── R-type ──────────────────────────────────────────────────────────
        0x33 => {
            let a = regs.read(f::rs1(word));
            let b = regs.read(f::rs2(word));
            let v = match (f::funct3(word), f::funct7(word)) {
                (0x0, 0x00) => a.wrapping_add(b),
                (0x0, 0x20) => a.wrapping_sub(b),
                (0x1, _) => a << (b & 0x1F),
                (0x2, _) => ((a as i32) < (b as i32)) as u32,
                (0x3, _) => (a < b) as u32,
                (0x4, _) => a ^ b,
                (0x5, 0x00) => a >> (b & 0x1F),
                (0x5, 0x20) => ((a as i32) >> (b & 0x1F)) as u32,
                (0x6, _) => a | b,
                (0x7, _) => a & b,
                _ => return Err(illegal(pc, word)),
            };
            regs.write(f::rd(word), v);
            Ok(StepResult::Next(pc.wrapping_add(4)))
        }

        // ── I-type arithmetic ────────────────────────────────────────────────
        0x13 => {
            let a = regs.read(f::rs1(word));
            let imm = f::imm_i(word);
            let v = match f::funct3(word) {
                0x0 => a.wrapping_add(imm as u32),
                0x2 => ((a as i32) < imm) as u32,
                0x3 => (a < imm as u32) as u32,
                0x4 => a ^ (imm as u32),
                0x6 => a | (imm as u32),
                0x7 => a & (imm as u32),
                0x1 => a << (imm & 0x1F) as u32,
                0x5 => {
                    let shamt = (imm as u32) & 0x1F;
                    if f::funct7(word) == 0x20 {
                        ((a as i32) >> shamt) as u32
                    } else {
                        a >> shamt
                    }
                }
                _ => return Err(illegal(pc, word)),
            };
            regs.write(f::rd(word), v);
            Ok(StepResult::Next(pc.wrapping_add(4)))
        }

        // ── Load ─────────────────────────────────────────────────────────────
        0x03 => {
            let addr = (regs.read(f::rs1(word)) as i32).wrapping_add(f::imm_i(word)) as u32;
            let v = match f::funct3(word) {
                0x0 => mem.load_byte(addr) as i8 as i32 as u32,
                0x1 => mem.load_halfword(addr) as i16 as i32 as u32,
                0x2 => mem.load_word(addr),
                0x4 => mem.load_byte(addr) as u32,
                0x5 => mem.load_halfword(addr) as u32,
                _ => return Err(illegal(pc, word)),
            };
            regs.write(f::rd(word), v);
            Ok(StepResult::Next(pc.wrapping_add(4)))
        }

        // ── Store ─────────────────────────────────────────────────────────────
        0x23 => {
            let addr = (regs.read(f::rs1(word)) as i32).wrapping_add(f::imm_s(word)) as u32;
            let val = regs.read(f::rs2(word));
            match f::funct3(word) {
                0x0 => mem.store_byte(addr, val as u8),
                0x1 => mem.store_halfword(addr, val as u16),
                0x2 => mem.store_word(addr, val),
                _ => return Err(illegal(pc, word)),
            }
            Ok(StepResult::Next(pc.wrapping_add(4)))
        }

        // ── Branch ────────────────────────────────────────────────────────────
        0x63 => {
            let a = regs.read(f::rs1(word));
            let b = regs.read(f::rs2(word));
            let taken = match f::funct3(word) {
                0x0 => a == b,
                0x1 => a != b,
                0x4 => (a as i32) < (b as i32),
                0x5 => (a as i32) >= (b as i32),
                0x6 => a < b,
                0x7 => a >= b,
                _ => return Err(illegal(pc, word)),
            };
            let next = if taken {
                (pc as i32).wrapping_add(f::imm_b(word)) as u32
            } else {
                pc.wrapping_add(4)
            };
            Ok(StepResult::Next(next))
        }

        // ── JAL ───────────────────────────────────────────────────────────────
        0x6F => {
            let target = (pc as i32).wrapping_add(f::imm_j(word)) as u32;
            regs.write(f::rd(word), pc.wrapping_add(4));
            Ok(StepResult::Next(target))
        }

        // ── JALR ──────────────────────────────────────────────────────────────
        0x67 => {
            let target =
                ((regs.read(f::rs1(word)) as i32).wrapping_add(f::imm_i(word)) as u32) & !1;
            regs.write(f::rd(word), pc.wrapping_add(4));
            Ok(StepResult::Next(target))
        }

        // ── LUI ───────────────────────────────────────────────────────────────
        0x37 => {
            regs.write(f::rd(word), f::imm_u(word));
            Ok(StepResult::Next(pc.wrapping_add(4)))
        }

        // ── AUIPC ─────────────────────────────────────────────────────────────
        0x17 => {
            regs.write(f::rd(word), pc.wrapping_add(f::imm_u(word)));
            Ok(StepResult::Next(pc.wrapping_add(4)))
        }

        // ── FENCE: treat as NOP ───────────────────────────────────────────────
        0x0F => Ok(StepResult::Next(pc.wrapping_add(4))),

        // ── SYSTEM ───────────────────────────────────────────────────────────
        0x73 => match f::imm_i(word) as u32 {
            0x000 => Ok(StepResult::Ecall),
            0x001 => Ok(StepResult::Ebreak),
            _ => Err(illegal(pc, word)),
        },

        _ => Err(illegal(pc, word)),
    }
}

fn illegal(pc: u32, word: u32) -> OarsError {
    OarsError::Runtime {
        pc,
        msg: format!("illegal instruction {word:#010x}"),
    }
}
