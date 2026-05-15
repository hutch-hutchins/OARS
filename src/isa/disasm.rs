use crate::isa::formats::{
    funct3, funct7, imm_b, imm_i, imm_j, imm_s, imm_u, opcode, rd, rs1, rs2,
};

const XREG: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6",
];
const FREG: [&str; 32] = [
    "ft0", "ft1", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7", "fs0", "fs1", "fa0", "fa1", "fa2",
    "fa3", "fa4", "fa5", "fa6", "fa7", "fs2", "fs3", "fs4", "fs5", "fs6", "fs7", "fs8", "fs9",
    "fs10", "fs11", "ft8", "ft9", "ft10", "ft11",
];

#[inline]
fn x(n: usize) -> &'static str {
    XREG[n & 31]
}
#[inline]
fn f(n: usize) -> &'static str {
    FREG[n & 31]
}

fn csr_name(csr: u32) -> String {
    match csr {
        0x001 => "fflags".into(),
        0x002 => "frm".into(),
        0x003 => "fcsr".into(),
        0xC00 => "cycle".into(),
        0xC01 => "time".into(),
        0xC02 => "instret".into(),
        0xC80 => "cycleh".into(),
        0xC82 => "instreth".into(),
        n => format!("{n:#06x}"),
    }
}

/// Decode a 32-bit RISC-V word to a human-readable mnemonic string.
/// `addr` is the instruction's address, used to compute branch/jump targets.
pub fn disassemble(word: u32, addr: u32) -> String {
    // Common pseudo-instructions
    if word == 0x0000_0013 {
        return "nop".into();
    }
    if word == 0x0000_8067 {
        return "ret".into();
    }
    if word == 0 {
        return "unimp".into();
    }

    let opc = opcode(word);
    let d = rd(word);
    let s1 = rs1(word);
    let s2 = rs2(word);
    let f3 = funct3(word);
    let f7 = funct7(word);
    let ii = imm_i(word);
    let is = imm_s(word);
    let ib = imm_b(word);
    let iu = imm_u(word);
    let ij = imm_j(word);

    match opc {
        // ── OP (R-type) ──────────────────────────────────────────────────────
        0x33 => {
            let mnem = match (f3, f7) {
                (0, 0x00) => "add",
                (0, 0x20) => "sub",
                (1, 0x00) => "sll",
                (2, 0x00) => "slt",
                (3, 0x00) => "sltu",
                (4, 0x00) => "xor",
                (5, 0x00) => "srl",
                (5, 0x20) => "sra",
                (6, 0x00) => "or",
                (7, 0x00) => "and",
                (0, 0x01) => "mul",
                (1, 0x01) => "mulh",
                (2, 0x01) => "mulhsu",
                (3, 0x01) => "mulhu",
                (4, 0x01) => "div",
                (5, 0x01) => "divu",
                (6, 0x01) => "rem",
                (7, 0x01) => "remu",
                _ => return format!("{word:#010x}"),
            };
            if mnem == "add" && s2 == 0 {
                return format!("mv      {}, {}", x(d), x(s1));
            }
            if mnem == "sub" && s1 == 0 {
                return format!("neg     {}, {}", x(d), x(s2));
            }
            format!("{mnem:<8}{}, {}, {}", x(d), x(s1), x(s2))
        }

        // ── OP-IMM (I-type) ──────────────────────────────────────────────────
        0x13 => {
            let shamt = (word >> 20) & 0x3F; // 6-bit for RV64 compatibility
            let mnem = match f3 {
                0 => "addi",
                1 => "slli",
                2 => "slti",
                3 => "sltiu",
                4 => "xori",
                5 => {
                    if f7 & 0x20 != 0 {
                        "srai"
                    } else {
                        "srli"
                    }
                }
                6 => "ori",
                7 => "andi",
                _ => unreachable!(),
            };
            // Pseudo: li rd, imm  (addi rd, zero, imm)
            if mnem == "addi" && s1 == 0 {
                return format!("li      {}, {}", x(d), ii);
            }
            // Pseudo: not rd, rs  (xori rd, rs, -1)
            if mnem == "xori" && ii == -1 {
                return format!("not     {}, {}", x(d), x(s1));
            }
            let imm_disp = if f3 == 1 || f3 == 5 { shamt as i32 } else { ii };
            format!("{mnem:<8}{}, {}, {}", x(d), x(s1), imm_disp)
        }

        // ── LOAD (I-type) ────────────────────────────────────────────────────
        0x03 => {
            let mnem = match f3 {
                0 => "lb",
                1 => "lh",
                2 => "lw",
                3 => "ld",
                4 => "lbu",
                5 => "lhu",
                6 => "lwu",
                _ => return format!("{word:#010x}"),
            };
            format!("{mnem:<8}{}, {}({})", x(d), ii, x(s1))
        }

        // ── STORE (S-type) ───────────────────────────────────────────────────
        0x23 => {
            let mnem = match f3 {
                0 => "sb",
                1 => "sh",
                2 => "sw",
                3 => "sd",
                _ => return format!("{word:#010x}"),
            };
            format!("{mnem:<8}{}, {}({})", x(s2), is, x(s1))
        }

        // ── BRANCH (B-type) ──────────────────────────────────────────────────
        0x63 => {
            let target = addr.wrapping_add(ib as u32);
            let mnem = match f3 {
                0 => "beq",
                1 => "bne",
                4 => "blt",
                5 => "bge",
                6 => "bltu",
                7 => "bgeu",
                _ => return format!("{word:#010x}"),
            };
            // Single-register pseudo branches
            match (f3, s2, s1) {
                (0, 0, _) => return format!("beqz    {}, {:#010x}", x(s1), target),
                (1, 0, _) => return format!("bnez    {}, {:#010x}", x(s1), target),
                (4, 0, _) => return format!("bltz    {}, {:#010x}", x(s1), target),
                (5, 0, _) => return format!("bgez    {}, {:#010x}", x(s1), target),
                (4, _, 0) => return format!("bgtz    {}, {:#010x}", x(s2), target),
                (5, _, 0) => return format!("blez    {}, {:#010x}", x(s2), target),
                _ => {}
            }
            format!("{mnem:<8}{}, {}, {:#010x}", x(s1), x(s2), target)
        }

        // ── JAL (J-type) ─────────────────────────────────────────────────────
        0x6F => {
            let target = addr.wrapping_add(ij as u32);
            match d {
                0 => format!("j       {:#010x}", target),
                1 => format!("call    {:#010x}", target),
                _ => format!("jal     {}, {:#010x}", x(d), target),
            }
        }

        // ── JALR (I-type) ────────────────────────────────────────────────────
        0x67 => {
            if d == 0 && s1 == 1 && ii == 0 {
                return "ret".into();
            }
            if d == 0 && ii == 0 {
                return format!("jr      {}", x(s1));
            }
            format!("jalr    {}, {}({})", x(d), ii, x(s1))
        }

        // ── LUI / AUIPC (U-type) ─────────────────────────────────────────────
        0x37 => format!("lui     {}, {:#07x}", x(d), iu >> 12),
        0x17 => format!("auipc   {}, {:#07x}", x(d), iu >> 12),

        // ── SYSTEM ───────────────────────────────────────────────────────────
        0x73 => {
            if f3 == 0 {
                return match ii {
                    0 => "ecall".into(),
                    1 => "ebreak".into(),
                    _ => format!("{word:#010x}"),
                };
            }
            let csr = word >> 20;
            let mnem = match f3 {
                1 => "csrrw",
                2 => "csrrs",
                3 => "csrrc",
                5 => "csrrwi",
                6 => "csrrsi",
                7 => "csrrci",
                _ => return format!("{word:#010x}"),
            };
            if f3 >= 5 {
                return format!("{mnem:<8}{}, {}, {}", x(d), csr_name(csr), s1);
            }
            if mnem == "csrrs" && s1 == 0 {
                return format!("csrr    {}, {}", x(d), csr_name(csr));
            }
            format!("{mnem:<8}{}, {}, {}", x(d), csr_name(csr), x(s1))
        }

        // ── LOAD-FP / STORE-FP ───────────────────────────────────────────────
        0x07 => {
            let m = match f3 {
                2 => "flw",
                3 => "fld",
                _ => return format!("{word:#010x}"),
            };
            format!("{m:<8}{}, {}({})", f(d), ii, x(s1))
        }
        0x27 => {
            let m = match f3 {
                2 => "fsw",
                3 => "fsd",
                _ => return format!("{word:#010x}"),
            };
            format!("{m:<8}{}, {}({})", f(s2), is, x(s1))
        }

        // ── OP-FP ────────────────────────────────────────────────────────────
        0x53 => {
            let sfx = if f7 & 1 == 0 { "s" } else { "d" };
            match f7 {
                0x00 | 0x01 => format!("fadd.{sfx}  {}, {}, {}", f(d), f(s1), f(s2)),
                0x04 | 0x05 => format!("fsub.{sfx}  {}, {}, {}", f(d), f(s1), f(s2)),
                0x08 | 0x09 => format!("fmul.{sfx}  {}, {}, {}", f(d), f(s1), f(s2)),
                0x0C | 0x0D => format!("fdiv.{sfx}  {}, {}, {}", f(d), f(s1), f(s2)),
                0x10 | 0x11 => {
                    let m = match f3 {
                        0 => "fsgnj",
                        1 => "fsgnjn",
                        2 => "fsgnjx",
                        _ => return format!("{word:#010x}"),
                    };
                    format!("{m}.{sfx}  {}, {}, {}", f(d), f(s1), f(s2))
                }
                0x14 | 0x15 => {
                    let m = match f3 {
                        0 => "fmin",
                        1 => "fmax",
                        _ => return format!("{word:#010x}"),
                    };
                    format!("{m}.{sfx}   {}, {}, {}", f(d), f(s1), f(s2))
                }
                0x20 => format!("fcvt.s.d  {}, {}", f(d), f(s1)),
                0x21 => format!("fcvt.d.s  {}, {}", f(d), f(s1)),
                0x2C | 0x2D => format!("fsqrt.{sfx}  {}, {}", f(d), f(s1)),
                0x50 | 0x51 => {
                    let m = match f3 {
                        0 => "fle",
                        1 => "flt",
                        2 => "feq",
                        _ => return format!("{word:#010x}"),
                    };
                    format!("{m}.{sfx}   {}, {}, {}", x(d), f(s1), f(s2))
                }
                0x60 | 0x61 => {
                    let t = if s2 == 0 { "w" } else { "wu" };
                    format!("fcvt.{t}.{sfx}  {}, {}", x(d), f(s1))
                }
                0x68 | 0x69 => {
                    let t = if s2 == 0 { "w" } else { "wu" };
                    format!("fcvt.{sfx}.{t}  {}, {}", f(d), x(s1))
                }
                0x70 | 0x71 => {
                    if f3 == 0 {
                        format!("fmv.x.{sfx}  {}, {}", x(d), f(s1))
                    } else {
                        format!("fclass.{sfx}  {}, {}", x(d), f(s1))
                    }
                }
                0x78 | 0x79 => format!("fmv.{sfx}.x  {}, {}", f(d), x(s1)),
                _ => format!("{word:#010x}"),
            }
        }

        // ── FMADD / FMSUB / FNMSUB / FNMADD ─────────────────────────────────
        0x43 | 0x47 | 0x4B | 0x4F => {
            let sfx = if (word >> 25) & 3 == 0 { "s" } else { "d" };
            let rs3 = ((word >> 27) & 0x1F) as usize;
            let m = match opc {
                0x43 => "fmadd",
                0x47 => "fmsub",
                0x4B => "fnmsub",
                _ => "fnmadd",
            };
            format!("{m}.{sfx}  {}, {}, {}, {}", f(d), f(s1), f(s2), f(rs3))
        }

        // ── RV64 OP-32 / OP-IMM-32 ──────────────────────────────────────────
        0x3B => {
            let mnem = match (f3, f7) {
                (0, 0x00) => "addw",
                (0, 0x20) => "subw",
                (1, 0x00) => "sllw",
                (5, 0x00) => "srlw",
                (5, 0x20) => "sraw",
                (0, 0x01) => "mulw",
                (4, 0x01) => "divw",
                (5, 0x01) => "divuw",
                (6, 0x01) => "remw",
                (7, 0x01) => "remuw",
                _ => return format!("{word:#010x}"),
            };
            format!("{mnem:<8}{}, {}, {}", x(d), x(s1), x(s2))
        }
        0x1B => {
            let shamt = (word >> 20) & 0x1F;
            let mnem = match f3 {
                0 => "addiw",
                1 => "slliw",
                5 => {
                    if f7 & 0x20 != 0 {
                        "sraiw"
                    } else {
                        "srliw"
                    }
                }
                _ => return format!("{word:#010x}"),
            };
            let disp = if f3 == 0 { ii } else { shamt as i32 };
            format!("{mnem:<8}{}, {}, {}", x(d), x(s1), disp)
        }

        _ => format!("{word:#010x}  # opcode {opc:#04x}"),
    }
}

// ─── Instruction reference lookup ────────────────────────────────────────────

/// Return `(description, example)` for a base mnemonic, if known.
/// The mnemonic is matched case-insensitively without operand suffixes.
pub fn describe(mnem: &str) -> Option<(&'static str, &'static str)> {
    // (mnemonic, description, example)
    static TABLE: &[(&str, &str, &str)] = &[
        // ── Pseudo ────────────────────────────────────────────────────────────
        ("nop", "No operation (addi zero, zero, 0)", "nop"),
        ("mv", "Copy register: rd = rs", "mv   t0, a0"),
        (
            "li",
            "Load immediate: rd = imm  (expands to lui+addi)",
            "li   a0, 42",
        ),
        (
            "not",
            "Bitwise NOT: rd = ~rs  (xori rd, rs, -1)",
            "not  t0, t1",
        ),
        ("neg", "Negate: rd = -rs  (sub rd, zero, rs)", "neg  t0, t1"),
        ("j", "Unconditional jump  (jal zero, offset)", "j    loop"),
        ("jr", "Jump register  (jalr zero, 0(rs))", "jr   ra"),
        ("ret", "Return  (jalr zero, 0(ra))", "ret"),
        ("call", "Call subroutine  (auipc+jalr ra)", "call my_fn"),
        (
            "beqz",
            "Branch if rs == 0  (beq rs, zero, label)",
            "beqz t0, done",
        ),
        (
            "bnez",
            "Branch if rs != 0  (bne rs, zero, label)",
            "bnez t0, loop",
        ),
        (
            "bltz",
            "Branch if rs < 0  (blt rs, zero, label)",
            "bltz t0, neg",
        ),
        (
            "bgez",
            "Branch if rs >= 0  (bge rs, zero, label)",
            "bgez t0, ok",
        ),
        (
            "bgtz",
            "Branch if rs > 0  (blt zero, rs, label)",
            "bgtz a0, pos",
        ),
        (
            "blez",
            "Branch if rs <= 0  (bge zero, rs, label)",
            "blez a0, done",
        ),
        // ── RV32I Base ────────────────────────────────────────────────────────
        ("add", "rd = rs1 + rs2", "add  t0, t1, t2"),
        ("sub", "rd = rs1 - rs2", "sub  t0, t1, t2"),
        ("and", "rd = rs1 & rs2  (bitwise AND)", "and  t0, t1, t2"),
        ("or", "rd = rs1 | rs2  (bitwise OR)", "or   t0, t1, t2"),
        ("xor", "rd = rs1 ^ rs2  (bitwise XOR)", "xor  t0, t1, t2"),
        (
            "sll",
            "rd = rs1 << rs2  (logical left shift)",
            "sll  t0, t1, t2",
        ),
        (
            "srl",
            "rd = rs1 >> rs2  (logical right shift)",
            "srl  t0, t1, t2",
        ),
        (
            "sra",
            "rd = rs1 >> rs2  (arithmetic right shift)",
            "sra  t0, t1, t2",
        ),
        (
            "slt",
            "rd = (rs1 < rs2) ? 1 : 0  (signed)",
            "slt  t0, t1, t2",
        ),
        (
            "sltu",
            "rd = (rs1 < rs2) ? 1 : 0  (unsigned)",
            "sltu t0, t1, t2",
        ),
        (
            "addi",
            "rd = rs1 + imm  (sign-extended 12-bit immediate)",
            "addi t0, t1, 4",
        ),
        ("andi", "rd = rs1 & imm", "andi t0, t1, 0xFF"),
        ("ori", "rd = rs1 | imm", "ori  t0, t1, 1"),
        ("xori", "rd = rs1 ^ imm", "xori t0, t1, -1"),
        (
            "slti",
            "rd = (rs1 < imm) ? 1 : 0  (signed)",
            "slti t0, t1, 10",
        ),
        (
            "sltiu",
            "rd = (rs1 < imm) ? 1 : 0  (unsigned)",
            "sltiu t0, t1, 10",
        ),
        (
            "slli",
            "rd = rs1 << shamt  (logical left shift immediate)",
            "slli t0, t0, 2",
        ),
        (
            "srli",
            "rd = rs1 >> shamt  (logical right shift imm)",
            "srli t0, t0, 1",
        ),
        (
            "srai",
            "rd = rs1 >> shamt  (arithmetic right shift imm)",
            "srai t0, t0, 31",
        ),
        ("lb", "rd = sign-extend(mem8[rs1+imm])", "lb   t0, 0(a0)"),
        ("lh", "rd = sign-extend(mem16[rs1+imm])", "lh   t0, 0(a0)"),
        ("lw", "rd = mem32[rs1+imm]", "lw   t0, 0(a0)"),
        ("lbu", "rd = zero-extend(mem8[rs1+imm])", "lbu  t0, 0(a0)"),
        ("lhu", "rd = zero-extend(mem16[rs1+imm])", "lhu  t0, 0(a0)"),
        ("sb", "mem8[rs1+imm] = rs2[7:0]", "sb   t0, 0(a0)"),
        ("sh", "mem16[rs1+imm] = rs2[15:0]", "sh   t0, 0(a0)"),
        ("sw", "mem32[rs1+imm] = rs2", "sw   t0, 0(a0)"),
        (
            "beq",
            "Branch to PC+offset if rs1 == rs2",
            "beq  t0, t1, done",
        ),
        (
            "bne",
            "Branch to PC+offset if rs1 != rs2",
            "bne  t0, t1, loop",
        ),
        (
            "blt",
            "Branch to PC+offset if rs1 < rs2  (signed)",
            "blt  t0, t1, neg",
        ),
        (
            "bge",
            "Branch to PC+offset if rs1 >= rs2  (signed)",
            "bge  t0, t1, ok",
        ),
        (
            "bltu",
            "Branch to PC+offset if rs1 < rs2  (unsigned)",
            "bltu t0, t1, wrap",
        ),
        (
            "bgeu",
            "Branch to PC+offset if rs1 >= rs2  (unsigned)",
            "bgeu t0, t1, ok",
        ),
        (
            "jal",
            "rd = PC+4; PC += offset  (jump and link)",
            "jal  ra, my_fn",
        ),
        (
            "jalr",
            "rd = PC+4; PC = rs1+imm  (jump and link register)",
            "jalr zero, 0(ra)",
        ),
        (
            "lui",
            "rd = imm << 12  (load upper immediate)",
            "lui  t0, 0x12345",
        ),
        (
            "auipc",
            "rd = PC + (imm << 12)  (add upper imm to PC)",
            "auipc t0, 0",
        ),
        (
            "ecall",
            "Environment call (syscall); a7 selects service",
            "ecall",
        ),
        ("ebreak", "Environment break (debugger trap)", "ebreak"),
        // ── RV32M ─────────────────────────────────────────────────────────────
        (
            "mul",
            "rd = (rs1 × rs2)[31:0]  (lower 32 bits)",
            "mul  t0, t1, t2",
        ),
        (
            "mulh",
            "rd = (rs1 × rs2)[63:32]  (signed × signed, upper)",
            "mulh t0, t1, t2",
        ),
        (
            "mulhsu",
            "rd = (rs1 × rs2)[63:32]  (signed × unsigned)",
            "mulhsu t0, t1, t2",
        ),
        (
            "mulhu",
            "rd = (rs1 × rs2)[63:32]  (unsigned × unsigned)",
            "mulhu t0, t1, t2",
        ),
        (
            "div",
            "rd = rs1 / rs2  (signed integer division)",
            "div  t0, t1, t2",
        ),
        (
            "divu",
            "rd = rs1 / rs2  (unsigned integer division)",
            "divu t0, t1, t2",
        ),
        (
            "rem",
            "rd = rs1 % rs2  (signed remainder)",
            "rem  t0, t1, t2",
        ),
        (
            "remu",
            "rd = rs1 % rs2  (unsigned remainder)",
            "remu t0, t1, t2",
        ),
        // ── RV32F ─────────────────────────────────────────────────────────────
        (
            "flw",
            "fd = float32 load from mem[rs1+imm]",
            "flw  ft0, 0(a0)",
        ),
        ("fsw", "mem[rs1+imm] = float32 store fs2", "fsw  ft0, 0(a0)"),
        (
            "fadd.s",
            "fd = fs1 + fs2  (single-precision)",
            "fadd.s ft0,ft1,ft2",
        ),
        (
            "fsub.s",
            "fd = fs1 - fs2  (single-precision)",
            "fsub.s ft0,ft1,ft2",
        ),
        (
            "fmul.s",
            "fd = fs1 × fs2  (single-precision)",
            "fmul.s ft0,ft1,ft2",
        ),
        (
            "fdiv.s",
            "fd = fs1 / fs2  (single-precision)",
            "fdiv.s ft0,ft1,ft2",
        ),
        (
            "fsqrt.s",
            "fd = √fs1  (single-precision square root)",
            "fsqrt.s ft0, ft1",
        ),
        (
            "fmadd.s",
            "fd = fs1×fs2 + fs3  (fused multiply-add, .s)",
            "fmadd.s ft0,ft1,ft2,ft3",
        ),
        (
            "fmsub.s",
            "fd = fs1×fs2 - fs3  (fused multiply-sub, .s)",
            "fmsub.s ft0,ft1,ft2,ft3",
        ),
        (
            "flt.s",
            "rd = (fs1 < fs2) ? 1 : 0  (float compare, .s)",
            "flt.s t0, ft0, ft1",
        ),
        (
            "fle.s",
            "rd = (fs1 <= fs2) ? 1 : 0  (float compare, .s)",
            "fle.s t0, ft0, ft1",
        ),
        (
            "feq.s",
            "rd = (fs1 == fs2) ? 1 : 0  (float compare, .s)",
            "feq.s t0, ft0, ft1",
        ),
        (
            "fcvt.w.s",
            "rd = (int32)fs1  (float→signed int, truncate)",
            "fcvt.w.s t0, ft0",
        ),
        (
            "fcvt.wu.s",
            "rd = (uint32)fs1  (float→unsigned int)",
            "fcvt.wu.s t0, ft0",
        ),
        (
            "fcvt.s.w",
            "fd = (float)rs1  (signed int→float)",
            "fcvt.s.w ft0, t0",
        ),
        (
            "fcvt.s.wu",
            "fd = (float)(uint)rs1  (unsigned int→float)",
            "fcvt.s.wu ft0, t0",
        ),
        (
            "fmv.x.s",
            "rd = bit-cast float register to int",
            "fmv.x.s t0, ft0",
        ),
        (
            "fmv.s.x",
            "fd = bit-cast int register to float",
            "fmv.s.x ft0, t0",
        ),
        // ── RV32D ─────────────────────────────────────────────────────────────
        (
            "fld",
            "fd = float64 load from mem[rs1+imm]",
            "fld  ft0, 0(a0)",
        ),
        ("fsd", "mem[rs1+imm] = float64 store fs2", "fsd  ft0, 0(a0)"),
        (
            "fadd.d",
            "fd = fs1 + fs2  (double-precision)",
            "fadd.d ft0,ft1,ft2",
        ),
        (
            "fsub.d",
            "fd = fs1 - fs2  (double-precision)",
            "fsub.d ft0,ft1,ft2",
        ),
        (
            "fmul.d",
            "fd = fs1 × fs2  (double-precision)",
            "fmul.d ft0,ft1,ft2",
        ),
        (
            "fdiv.d",
            "fd = fs1 / fs2  (double-precision)",
            "fdiv.d ft0,ft1,ft2",
        ),
        (
            "fsqrt.d",
            "fd = √fs1  (double-precision square root)",
            "fsqrt.d ft0, ft1",
        ),
        (
            "fcvt.d.s",
            "fd = (double)fs1  (single→double)",
            "fcvt.d.s ft0, ft1",
        ),
        (
            "fcvt.s.d",
            "fd = (float)fs1   (double→single)",
            "fcvt.s.d ft0, ft1",
        ),
        (
            "fcvt.w.d",
            "rd = (int32)fs1  (double→signed int)",
            "fcvt.w.d t0, ft0",
        ),
        (
            "fcvt.d.w",
            "fd = (double)rs1  (signed int→double)",
            "fcvt.d.w ft0, t0",
        ),
        // ── Zicsr ─────────────────────────────────────────────────────────────
        (
            "csrrw",
            "rd = CSR; CSR = rs1  (read/write CSR)",
            "csrrw t0, cycle, zero",
        ),
        (
            "csrrs",
            "rd = CSR; CSR |= rs1  (read/set bits in CSR)",
            "csrrs t0, instret, zero",
        ),
        (
            "csrrc",
            "rd = CSR; CSR &= ~rs1  (read/clear bits)",
            "csrrc t0, fflags, t0",
        ),
        (
            "csrrwi",
            "rd = CSR; CSR = uimm  (immediate write)",
            "csrrwi t0, frm, 0",
        ),
        (
            "csrrsi",
            "rd = CSR; CSR |= uimm  (immediate set bits)",
            "csrrsi t0, fflags, 1",
        ),
        (
            "csrrci",
            "rd = CSR; CSR &= ~uimm  (immediate clear bits)",
            "csrrci t0, fflags, 1",
        ),
        (
            "csrr",
            "rd = CSR  (read CSR, alias for csrrs rd, csr, zero)",
            "csrr t0, cycle",
        ),
        // ── RV64I ─────────────────────────────────────────────────────────────
        ("ld", "rd = mem64[rs1+imm]  (64-bit load)", "ld   t0, 0(a0)"),
        (
            "sd",
            "mem64[rs1+imm] = rs2  (64-bit store)",
            "sd   t0, 0(a0)",
        ),
        (
            "lwu",
            "rd = zero-extend(mem32[rs1+imm])  (RV64)",
            "lwu  t0, 0(a0)",
        ),
        (
            "addiw",
            "rd = sign-extend32(rs1[31:0] + imm)",
            "addiw t0, t0, 1",
        ),
        (
            "addw",
            "rd = sign-extend32(rs1[31:0] + rs2[31:0])",
            "addw t0, t1, t2",
        ),
        (
            "subw",
            "rd = sign-extend32(rs1[31:0] - rs2[31:0])",
            "subw t0, t1, t2",
        ),
        (
            "sllw",
            "rd = sign-extend32(rs1 << rs2[4:0])",
            "sllw t0, t1, t2",
        ),
        (
            "srlw",
            "rd = sign-extend32(rs1[31:0] >> rs2[4:0])",
            "srlw t0, t1, t2",
        ),
        (
            "sraw",
            "rd = sign-extend32(rs1[31:0] >>> rs2[4:0])",
            "sraw t0, t1, t2",
        ),
        (
            "mulhu",
            "rd = upper 64 bits of rs1 × rs2  (unsigned)",
            "mulhu t0, t1, t2",
        ),
        (
            "mulw",
            "rd = sign-extend32(rs1[31:0] × rs2[31:0])",
            "mulw t0, t1, t2",
        ),
    ];

    let key = mnem.to_lowercase();
    TABLE
        .iter()
        .find(|(m, _, _)| *m == key)
        .map(|(_, d, e)| (*d, *e))
}
