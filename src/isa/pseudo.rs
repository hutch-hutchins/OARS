/// Pseudo-instruction expansion.
///
/// Each pseudo-instruction expands to one or two real instructions.
/// The assembler calls `expand()` after parsing the mnemonic.
///
/// Returned instructions are expressed as `RealInstr` — a mnemonic + operands
/// that the code generator can then encode directly.

#[derive(Debug, Clone)]
pub enum Operand {
    Reg(usize),
    Imm(i32),
    Label(String),
    MemOff(i32, usize), // offset(reg)
}

#[derive(Debug, Clone)]
pub struct RealInstr {
    pub mnemonic: &'static str,
    pub ops: Vec<Operand>,
}

impl RealInstr {
    fn new(mnemonic: &'static str, ops: Vec<Operand>) -> Self {
        Self { mnemonic, ops }
    }
}

/// Attempt to expand a pseudo-instruction.
/// Returns `None` if `mnemonic` is not a known pseudo-instruction — the caller
/// then treats it as a real instruction.
pub fn expand(mnemonic: &str, ops: &[Operand]) -> Option<Vec<RealInstr>> {
    use Operand::*;

    Some(match mnemonic {
        "nop" => vec![RealInstr::new("addi", vec![Reg(0), Reg(0), Imm(0)])],

        "mv" => {
            let (rd, rs) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("addi", vec![rd, rs, Imm(0)])]
        }

        "not" => {
            let (rd, rs) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("xori", vec![rd, rs, Imm(-1)])]
        }

        "neg" => {
            let (rd, rs) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("sub", vec![rd, Reg(0), rs])]
        }

        "seqz" => {
            let (rd, rs) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("sltiu", vec![rd, rs, Imm(1)])]
        }

        "snez" => {
            let (rd, rs) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("sltu", vec![rd, Reg(0), rs])]
        }

        "sltz" => {
            let (rd, rs) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("slt", vec![rd, rs, Reg(0)])]
        }

        "sgtz" => {
            let (rd, rs) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("slt", vec![rd, Reg(0), rs])]
        }

        // li rd, imm — single or double instruction
        "li" => {
            let rd = ops[0].clone();
            let imm = match &ops[1] { Imm(v) => *v, _ => return None };
            expand_li(rd, imm)
        }

        // la rd, label — lui + addi (resolved at codegen time)
        "la" => {
            let rd = ops[0].clone();
            let lbl = match &ops[1] { Label(s) => s.clone(), _ => return None };
            // Emit two placeholder instructions; codegen resolves the label.
            vec![
                RealInstr::new("lui",  vec![rd.clone(), Label(format!("%hi({})", lbl))]),
                RealInstr::new("addi", vec![rd.clone(), rd, Label(format!("%lo({})", lbl))]),
            ]
        }

        "j"    => vec![RealInstr::new("jal",  vec![Reg(0), ops[0].clone()])],
        "jr"   => vec![RealInstr::new("jalr", vec![Reg(0), ops[0].clone(), Imm(0)])],
        "ret"  => vec![RealInstr::new("jalr", vec![Reg(0), Reg(1), Imm(0)])],
        "call" => vec![RealInstr::new("jal",  vec![Reg(1), ops[0].clone()])],

        "beqz" => {
            let (rs, lbl) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("beq", vec![rs, Reg(0), lbl])]
        }
        "bnez" => {
            let (rs, lbl) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("bne", vec![rs, Reg(0), lbl])]
        }
        "blez" => {
            let (rs, lbl) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("bge", vec![Reg(0), rs, lbl])]
        }
        "bgez" => {
            let (rs, lbl) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("bge", vec![rs, Reg(0), lbl])]
        }
        "bltz" => {
            let (rs, lbl) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("blt", vec![rs, Reg(0), lbl])]
        }
        "bgtz" => {
            let (rs, lbl) = (ops[0].clone(), ops[1].clone());
            vec![RealInstr::new("blt", vec![Reg(0), rs, lbl])]
        }
        "bgt" => {
            let (rs, rt, lbl) = (ops[0].clone(), ops[1].clone(), ops[2].clone());
            vec![RealInstr::new("blt", vec![rt, rs, lbl])]
        }
        "ble" => {
            let (rs, rt, lbl) = (ops[0].clone(), ops[1].clone(), ops[2].clone());
            vec![RealInstr::new("bge", vec![rt, rs, lbl])]
        }
        "bgtu" => {
            let (rs, rt, lbl) = (ops[0].clone(), ops[1].clone(), ops[2].clone());
            vec![RealInstr::new("bltu", vec![rt, rs, lbl])]
        }
        "bleu" => {
            let (rs, rt, lbl) = (ops[0].clone(), ops[1].clone(), ops[2].clone());
            vec![RealInstr::new("bgeu", vec![rt, rs, lbl])]
        }

        _ => return None,
    })
}

fn expand_li(rd: Operand, imm: i32) -> Vec<RealInstr> {
    if imm >= -2048 && imm <= 2047 {
        vec![RealInstr::new("addi", vec![rd, Operand::Reg(0), Operand::Imm(imm)])]
    } else {
        // upper20 rounded so that sign-extended lower12 adds correctly
        let upper = ((imm as u32).wrapping_add(0x800)) >> 12;
        let lower = imm - ((upper as i32) << 12);
        vec![
            RealInstr::new("lui",  vec![rd.clone(), Operand::Imm(upper as i32)]),
            RealInstr::new("addi", vec![rd.clone(), rd, Operand::Imm(lower)]),
        ]
    }
}
