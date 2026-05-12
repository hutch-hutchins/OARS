/// RV32F + RV32D execution (single-precision and double-precision floating point).
use crate::hardware::{fp_registers::FpRegisters, memory::Memory, registers::RegisterFile};
use crate::isa::formats as f;
use crate::util::error::OarsError;

fn ill(pc: u32, word: u32) -> OarsError {
    OarsError::Runtime {
        pc,
        msg: format!("illegal FP instruction {word:#010x}"),
    }
}

/// Execute one FP instruction. Handles opcodes: 0x07 0x27 0x43 0x47 0x4B 0x4F 0x53.
pub fn step(
    word: u32,
    pc: u32,
    regs: &mut RegisterFile,
    fp: &mut FpRegisters,
    mem: &mut Memory,
) -> Result<u32, OarsError> {
    let opc = f::opcode(word);
    let rd = f::rd(word);
    let rs1 = f::rs1(word);
    let rs2 = f::rs2(word);
    let f3 = f::funct3(word);
    let f5 = (word >> 27) as usize; // bits 31:27 — operation selector
    let fmt = (word >> 25) & 0x3; // bits 26:25 — format: 0=S 1=D

    match opc {
        // ── FLW / FLD ─────────────────────────────────────────────────────────
        0x07 => {
            let addr = (regs.read(rs1) as i32).wrapping_add(f::imm_i(word)) as u32;
            match f3 {
                0x2 => fp.write_u32(rd, mem.load_word(addr)),
                0x3 => {
                    let lo = mem.load_word(addr) as u64;
                    let hi = mem.load_word(addr.wrapping_add(4)) as u64;
                    fp.write_u64(rd, lo | (hi << 32));
                }
                _ => return Err(ill(pc, word)),
            }
        }

        // ── FSW / FSD ─────────────────────────────────────────────────────────
        0x27 => {
            let addr = (regs.read(rs1) as i32).wrapping_add(f::imm_s(word)) as u32;
            match f3 {
                0x2 => mem.store_word(addr, fp.read_u32(rs2)),
                0x3 => {
                    let bits = fp.read_u64(rs2);
                    mem.store_word(addr, bits as u32);
                    mem.store_word(addr.wrapping_add(4), (bits >> 32) as u32);
                }
                _ => return Err(ill(pc, word)),
            }
        }

        // ── FMADD / FMSUB / FNMSUB / FNMADD ──────────────────────────────────
        0x43 | 0x47 | 0x4B | 0x4F => {
            let rs3 = f5; // bits 31:27 are rs3 for R4-type
            match fmt {
                0x0 => {
                    let (a, b, c) = (fp.read_f32(rs1), fp.read_f32(rs2), fp.read_f32(rs3));
                    let v = match opc {
                        0x43 => a.mul_add(b, c),
                        0x47 => a.mul_add(b, -c),
                        0x4B => (-a).mul_add(b, c),
                        0x4F => (-a).mul_add(b, -c),
                        _ => unreachable!(),
                    };
                    fp.write_f32(rd, v);
                }
                0x1 => {
                    let (a, b, c) = (fp.read_f64(rs1), fp.read_f64(rs2), fp.read_f64(rs3));
                    let v = match opc {
                        0x43 => a.mul_add(b, c),
                        0x47 => a.mul_add(b, -c),
                        0x4B => (-a).mul_add(b, c),
                        0x4F => (-a).mul_add(b, -c),
                        _ => unreachable!(),
                    };
                    fp.write_f64(rd, v);
                }
                _ => return Err(ill(pc, word)),
            }
        }

        // ── FP arithmetic (opcode 0x53) ───────────────────────────────────────
        0x53 => exec_arith(word, pc, regs, fp, rd, rs1, rs2, f3, f5, fmt)?,

        _ => return Err(ill(pc, word)),
    }

    Ok(pc.wrapping_add(4))
}

#[allow(clippy::too_many_arguments)]
fn exec_arith(
    word: u32,
    pc: u32,
    regs: &mut RegisterFile,
    fp: &mut FpRegisters,
    rd: usize,
    rs1: usize,
    rs2: usize,
    rm: u32,
    f5: usize,
    fmt: u32,
) -> Result<(), OarsError> {
    match (f5, fmt) {
        // ── Single precision ──────────────────────────────────────────────────
        (0x00, 0) => fp.write_f32(rd, fp.read_f32(rs1) + fp.read_f32(rs2)), // FADD.S
        (0x01, 0) => fp.write_f32(rd, fp.read_f32(rs1) - fp.read_f32(rs2)), // FSUB.S
        (0x02, 0) => fp.write_f32(rd, fp.read_f32(rs1) * fp.read_f32(rs2)), // FMUL.S
        (0x03, 0) => fp.write_f32(rd, fp.read_f32(rs1) / fp.read_f32(rs2)), // FDIV.S
        (0x0B, 0) => fp.write_f32(rd, fp.read_f32(rs1).sqrt()),             // FSQRT.S

        (0x04, 0) => {
            // FSGNJ.S / FSGNJN.S / FSGNJX.S
            let (a, b) = (fp.read_u32(rs1), fp.read_u32(rs2));
            fp.write_u32(
                rd,
                match rm {
                    0 => (a & 0x7FFF_FFFF) | (b & 0x8000_0000),
                    1 => (a & 0x7FFF_FFFF) | (!b & 0x8000_0000),
                    2 => (a & 0x7FFF_FFFF) | ((a ^ b) & 0x8000_0000),
                    _ => return Err(ill(pc, word)),
                },
            );
        }

        (0x05, 0) => fp.write_f32(
            rd,
            if rm == 0 {
                // FMIN.S / FMAX.S
                fp.read_f32(rs1).min(fp.read_f32(rs2))
            } else {
                fp.read_f32(rs1).max(fp.read_f32(rs2))
            },
        ),

        (0x14, 0) => {
            // FLE.S / FLT.S / FEQ.S
            let (a, b) = (fp.read_f32(rs1), fp.read_f32(rs2));
            regs.write(
                rd,
                match rm {
                    0 => (a <= b) as u32,
                    1 => (a < b) as u32,
                    _ => (a == b) as u32,
                },
            );
        }

        (0x18, 0) => {
            // FCVT.W.S (rs2=0) / FCVT.WU.S (rs2=1)
            let v = fp.read_f32(rs1);
            regs.write(rd, if rs2 == 0 { v as i32 as u32 } else { v as u32 });
        }

        (0x1A, 0) => {
            // FCVT.S.W (rs2=0) / FCVT.S.WU (rs2=1)
            fp.write_f32(
                rd,
                if rs2 == 0 {
                    regs.read(rs1) as i32 as f32
                } else {
                    regs.read(rs1) as f32
                },
            );
        }

        (0x1C, 0) => {
            // FMV.X.W (rm=0) / FCLASS.S (rm=1)
            if rm == 0 {
                regs.write(rd, fp.read_u32(rs1));
            } else {
                regs.write(rd, fclass32(fp.read_f32(rs1)));
            }
        }

        (0x1E, 0) => fp.write_u32(rd, regs.read(rs1)), // FMV.W.X

        // ── Conversion between S and D ─────────────────────────────────────────
        (0x20, 1) => fp.write_f32(rd, fp.read_f64(rs1) as f32), // FCVT.S.D
        (0x21, 0) => fp.write_f64(rd, fp.read_f32(rs1) as f64), // FCVT.D.S

        // ── Double precision ──────────────────────────────────────────────────
        (0x00, 1) => fp.write_f64(rd, fp.read_f64(rs1) + fp.read_f64(rs2)), // FADD.D
        (0x01, 1) => fp.write_f64(rd, fp.read_f64(rs1) - fp.read_f64(rs2)), // FSUB.D
        (0x02, 1) => fp.write_f64(rd, fp.read_f64(rs1) * fp.read_f64(rs2)), // FMUL.D
        (0x03, 1) => fp.write_f64(rd, fp.read_f64(rs1) / fp.read_f64(rs2)), // FDIV.D
        (0x0B, 1) => fp.write_f64(rd, fp.read_f64(rs1).sqrt()),             // FSQRT.D

        (0x04, 1) => {
            // FSGNJ.D / FSGNJN.D / FSGNJX.D
            let (a, b) = (fp.read_u64(rs1), fp.read_u64(rs2));
            let sign = 1u64 << 63;
            fp.write_u64(
                rd,
                match rm {
                    0 => (a & !sign) | (b & sign),
                    1 => (a & !sign) | (!b & sign),
                    2 => (a & !sign) | ((a ^ b) & sign),
                    _ => return Err(ill(pc, word)),
                },
            );
        }

        (0x05, 1) => fp.write_f64(
            rd,
            if rm == 0 {
                // FMIN.D / FMAX.D
                fp.read_f64(rs1).min(fp.read_f64(rs2))
            } else {
                fp.read_f64(rs1).max(fp.read_f64(rs2))
            },
        ),

        (0x14, 1) => {
            // FLE.D / FLT.D / FEQ.D
            let (a, b) = (fp.read_f64(rs1), fp.read_f64(rs2));
            regs.write(
                rd,
                match rm {
                    0 => (a <= b) as u32,
                    1 => (a < b) as u32,
                    _ => (a == b) as u32,
                },
            );
        }

        (0x18, 1) => {
            // FCVT.W.D / FCVT.WU.D
            let v = fp.read_f64(rs1);
            regs.write(rd, if rs2 == 0 { v as i32 as u32 } else { v as u32 });
        }

        (0x1A, 1) => {
            // FCVT.D.W / FCVT.D.WU
            fp.write_f64(
                rd,
                if rs2 == 0 {
                    regs.read(rs1) as i32 as f64
                } else {
                    regs.read(rs1) as f64
                },
            );
        }

        (0x10, 0) => regs.write(rd, fclass32(fp.read_f32(rs1))), // FCLASS.S (alt funct)
        (0x10, 1) => regs.write(rd, fclass64(fp.read_f64(rs1))), // FCLASS.D

        _ => {
            return Err(OarsError::Runtime {
                pc,
                msg: format!("unknown FP op f5={f5:#x} fmt={fmt}"),
            })
        }
    }
    Ok(())
}

// ─── FCLASS helpers ───────────────────────────────────────────────────────────

fn fclass32(v: f32) -> u32 {
    let b = v.to_bits();
    if v.is_nan() {
        if b & 0x0040_0000 != 0 {
            1 << 9
        } else {
            1 << 8
        }
    } else if v == f32::NEG_INFINITY {
        1
    } else if v == f32::INFINITY {
        1 << 7
    } else if b == 0x8000_0000 {
        1 << 3
    }
    // -0.0
    else if b == 0 {
        1 << 4
    }
    // +0.0
    else if v.is_subnormal() {
        if v < 0.0 {
            1 << 2
        } else {
            1 << 5
        }
    } else if v < 0.0 {
        1 << 1
    } else {
        1 << 6
    }
}

fn fclass64(v: f64) -> u32 {
    let b = v.to_bits();
    if v.is_nan() {
        if b & 0x0008_0000_0000_0000 != 0 {
            1 << 9
        } else {
            1 << 8
        }
    } else if v == f64::NEG_INFINITY {
        1
    } else if v == f64::INFINITY {
        1 << 7
    } else if b == 0x8000_0000_0000_0000 {
        1 << 3
    } else if b == 0 {
        1 << 4
    } else if v.is_subnormal() {
        if v < 0.0 {
            1 << 2
        } else {
            1 << 5
        }
    } else if v < 0.0 {
        1 << 1
    } else {
        1 << 6
    }
}
