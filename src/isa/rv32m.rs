use crate::hardware::registers::RegisterFile;
use crate::isa::formats as f;
use crate::util::error::OarsError;

/// Execute one RV32M instruction (opcode 0x33, funct7 = 0x01).
pub fn step(word: u32, pc: u32, regs: &mut RegisterFile) -> Result<u32, OarsError> {
    let a = regs.read(f::rs1(word));
    let b = regs.read(f::rs2(word));
    let rd = f::rd(word);

    let v = match f::funct3(word) {
        0x0 => a.wrapping_mul(b),                                      // MUL
        0x1 => (((a as i32 as i64) * (b as i32 as i64)) >> 32) as u32, // MULH
        0x2 => (((a as i32 as i64) * (b as u64 as i64)) >> 32) as u32, // MULHSU
        0x3 => (((a as u64) * (b as u64)) >> 32) as u32,               // MULHU
        0x4 => {
            // DIV (signed)
            if b == 0 {
                u32::MAX
            } else if a == 0x8000_0000 && b == 0xFFFF_FFFF {
                0x8000_0000
            }
            // overflow
            else {
                ((a as i32).wrapping_div(b as i32)) as u32
            }
        }
        0x5 => {
            if b == 0 {
                u32::MAX
            } else {
                a / b
            }
        } // DIVU
        0x6 => {
            // REM (signed)
            if b == 0 {
                a
            } else if a == 0x8000_0000 && b == 0xFFFF_FFFF {
                0
            } else {
                ((a as i32).wrapping_rem(b as i32)) as u32
            }
        }
        0x7 => {
            if b == 0 {
                a
            } else {
                a % b
            }
        } // REMU
        _ => {
            return Err(OarsError::Runtime {
                pc,
                msg: format!("illegal M-ext {word:#010x}"),
            })
        }
    };
    regs.write(rd, v);
    Ok(pc.wrapping_add(4))
}

/// Encode an RV32M instruction by mnemonic.
pub fn encode(mnemonic: &str, rd: u32, rs1: u32, rs2: u32) -> Option<u32> {
    let f3 = match mnemonic {
        "mul" => 0x0,
        "mulh" => 0x1,
        "mulhsu" => 0x2,
        "mulhu" => 0x3,
        "div" => 0x4,
        "divu" => 0x5,
        "rem" => 0x6,
        "remu" => 0x7,
        _ => return None,
    };
    Some(crate::isa::formats::enc_r(0x33, f3, 0x01, rd, rs1, rs2))
}
