use crate::assembler::parser::{DataItem, Instruction, Operand, Seg, Statement};
use crate::assembler::symbol_table::SymbolTable;
use crate::hardware::memory::{Memory, DATA_BASE, TEXT_BASE};
use crate::isa::formats as f;
use crate::isa::{pseudo, rv32m};
use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// One row in the text-segment view: assembled address, machine word, source line.
pub struct TextRow {
    pub addr: u32,
    pub word: u32,
    pub src_line: u32, // 1-based line number in the source file
}

/// Output of assembly: memory image + symbol table + entry PC + text-segment map.
pub struct AssemblyOutput {
    #[allow(dead_code)]
    pub symbols: SymbolTable,
    pub entry: u32,
    /// One past the last byte written into the text segment.
    pub text_end: u32,
    /// One past the last byte written into the data segment.
    pub data_end: u32,
    pub text_rows: Vec<TextRow>,
    /// Reverse map: address → labels defined at that address (for display).
    pub addr_to_labels: HashMap<u32, Vec<String>>,
}

/// Assemble a list of `Statement`s into `mem`.
/// Two-pass: pass 1 collects symbol addresses, pass 2 encodes instructions.
pub fn assemble(stmts: &[Statement], mem: &mut Memory) -> Result<AssemblyOutput> {
    let symbols = pass1(stmts);
    let addr_to_labels = build_addr_to_labels(&symbols);
    let (entry, text_end, data_end, text_rows) = pass2(stmts, &symbols, mem)?;
    Ok(AssemblyOutput {
        symbols,
        entry,
        text_end,
        data_end,
        text_rows,
        addr_to_labels,
    })
}

fn build_addr_to_labels(symbols: &SymbolTable) -> HashMap<u32, Vec<String>> {
    let mut map: HashMap<u32, Vec<String>> = HashMap::new();
    for (name, &addr) in symbols.all() {
        map.entry(addr).or_default().push(name.clone());
    }
    // Sort label lists for deterministic display order
    for labels in map.values_mut() {
        labels.sort();
    }
    map
}

// ─── Pass 1: collect labels ───────────────────────────────────────────────────

/// Pre-scan to collect all .equ / .set constants before label address calculation.
fn collect_equs(stmts: &[Statement]) -> HashMap<String, i32> {
    stmts
        .iter()
        .filter_map(|s| {
            if let Statement::Equ(name, val) = s {
                Some((name.clone(), *val))
            } else {
                None
            }
        })
        .collect()
}

fn pass1(stmts: &[Statement]) -> SymbolTable {
    let equs = collect_equs(stmts);
    let mut sym = SymbolTable::new();
    let mut text_pc = TEXT_BASE;
    let mut data_pc = DATA_BASE;
    let mut seg = Seg::Text;

    for stmt in stmts {
        match stmt {
            Statement::Segment(s, _) => seg = s.clone(),
            Statement::Label(name, _) => {
                sym.define(name, if seg == Seg::Text { text_pc } else { data_pc });
            }
            Statement::Instr(instr) => {
                text_pc += instr_size(instr, &equs) * 4;
            }
            Statement::Data(item, _) => {
                data_pc += data_item_size(item);
            }
            Statement::Globl(_) | Statement::Equ(_, _) | Statement::Include(_) => {}
        }
    }
    for (name, val) in &equs {
        sym.define_equ(name, *val);
    }
    sym
}

fn instr_size(instr: &Instruction, equs: &HashMap<String, i32>) -> u32 {
    match instr.mnemonic.as_str() {
        "li" => {
            let imm_val = match instr.ops.get(1) {
                Some(Operand::Imm(v)) => Some(*v),
                Some(Operand::Label(name)) => equs.get(name.as_str()).copied(),
                _ => None,
            };
            match imm_val {
                Some(v) if (-2048..=2047).contains(&v) => 1,
                Some(_) => 2,
                None => 2,
            }
        }
        "la" => 2,
        m => pseudo::expand(m, &conv_ops(&instr.ops))
            .map(|v| v.len() as u32)
            .unwrap_or(1),
    }
}

fn data_item_size(item: &DataItem) -> u32 {
    match item {
        DataItem::Byte(_) => 1,
        DataItem::Half(_) => 2,
        DataItem::Word(_) | DataItem::Float(_) => 4,
        DataItem::Dword(_) | DataItem::Double(_) => 8,
        DataItem::String(s) => s.len() as u32 + 1,
        DataItem::Ascii(s) => s.len() as u32,
        DataItem::Space(n) => *n,
        DataItem::Align(exp) => 1u32 << exp,
        DataItem::Words(v) => v.len() as u32 * 4,
        DataItem::Halfs(v) => v.len() as u32 * 2,
        DataItem::Bytes(v) => v.len() as u32,
        DataItem::Dwords(v) => v.len() as u32 * 8,
    }
}

// ─── Pass 2: encode instructions + data ──────────────────────────────────────

fn pass2(
    stmts: &[Statement],
    sym: &SymbolTable,
    mem: &mut Memory,
) -> Result<(u32, u32, u32, Vec<TextRow>)> {
    let mut text_pc = TEXT_BASE;
    let mut data_pc = DATA_BASE;
    let mut seg = Seg::Text;
    let entry = sym
        .resolve("main")
        .or_else(|| sym.resolve("_start"))
        .unwrap_or(TEXT_BASE);
    let mut text_rows: Vec<TextRow> = Vec::new();

    for stmt in stmts {
        match stmt {
            Statement::Segment(s, _) => seg = s.clone(),
            Statement::Label(_, _) => {}
            Statement::Instr(instr) => {
                let words = encode_instr(instr, text_pc, sym)?;
                for w in &words {
                    text_rows.push(TextRow {
                        addr: text_pc,
                        word: *w,
                        src_line: instr.span.line,
                    });
                    mem.store_word(text_pc, *w);
                    text_pc += 4;
                }
            }
            Statement::Data(item, _) => {
                data_pc = emit_data(item, data_pc, mem);
            }
            Statement::Globl(_) | Statement::Equ(_, _) | Statement::Include(_) => {}
        }
    }
    let _ = seg;
    Ok((entry, text_pc, data_pc, text_rows))
}

fn emit_data(item: &DataItem, addr: u32, mem: &mut Memory) -> u32 {
    match item {
        DataItem::Byte(v) => {
            mem.store_byte(addr, *v as u8);
            addr + 1
        }
        DataItem::Half(v) => {
            mem.store_halfword(addr, *v as u16);
            addr + 2
        }
        DataItem::Word(v) => {
            mem.store_word(addr, *v as u32);
            addr + 4
        }
        DataItem::Float(v) => {
            mem.store_word(addr, v.to_bits());
            addr + 4
        }
        DataItem::Double(v) => {
            let bits = v.to_bits();
            mem.store_word(addr, bits as u32);
            mem.store_word(addr + 4, (bits >> 32) as u32);
            addr + 8
        }
        DataItem::Dword(v) => {
            mem.store_doubleword(addr, *v as u64);
            addr + 8
        }
        DataItem::Dwords(vals) => {
            let mut a = addr;
            for v in vals {
                mem.store_doubleword(a, *v as u64);
                a += 8;
            }
            a
        }
        DataItem::String(s) => {
            let b = s.as_bytes();
            mem.write_bytes(addr, b);
            mem.store_byte(addr + b.len() as u32, 0);
            addr + b.len() as u32 + 1
        }
        DataItem::Ascii(s) => {
            let b = s.as_bytes();
            mem.write_bytes(addr, b);
            addr + b.len() as u32
        }
        DataItem::Space(n) => addr + n,
        DataItem::Align(exp) => {
            let a = 1u32 << exp;
            (addr + a - 1) & !(a - 1)
        }
        DataItem::Words(vals) => {
            let mut a = addr;
            for v in vals {
                mem.store_word(a, *v as u32);
                a += 4;
            }
            a
        }
        DataItem::Halfs(vals) => {
            let mut a = addr;
            for v in vals {
                mem.store_halfword(a, *v as u16);
                a += 2;
            }
            a
        }
        DataItem::Bytes(vals) => {
            let mut a = addr;
            for v in vals {
                mem.store_byte(a, *v as u8);
                a += 1;
            }
            a
        }
    }
}

// ─── Instruction encoder ──────────────────────────────────────────────────────

/// Substitute any .equ constant labels with their Imm values before pseudo expansion.
fn subst_equs(ops: &[Operand], sym: &SymbolTable) -> Vec<Operand> {
    ops.iter()
        .map(|o| {
            if let Operand::Label(name) = o {
                if let Some(val) = sym.resolve_equ(name) {
                    return Operand::Imm(val);
                }
            }
            o.clone()
        })
        .collect()
}

fn encode_instr(instr: &Instruction, pc: u32, sym: &SymbolTable) -> Result<Vec<u32>> {
    let subst = subst_equs(&instr.ops, sym);
    let pop_ops = conv_ops(&subst);
    if let Some(expanded) = pseudo::expand(&instr.mnemonic, &pop_ops) {
        let mut words = Vec::new();
        let mut cur = pc;
        for ri in expanded {
            let ops = resolve_pseudo_ops(&ri.ops, cur, sym)?;
            words.push(encode_real(ri.mnemonic, &ops, cur, sym)?);
            cur += 4;
        }
        return Ok(words);
    }
    let ops = resolve_ops(&subst, pc, sym)?;
    Ok(vec![encode_real(&instr.mnemonic, &ops, pc, sym)?])
}

/// Convert parser::Operand → pseudo::Operand.
fn conv_ops(ops: &[Operand]) -> Vec<pseudo::Operand> {
    ops.iter()
        .map(|o| match o {
            Operand::Reg(r) => pseudo::Operand::Reg(*r),
            Operand::FpReg(r) => pseudo::Operand::FpReg(*r),
            Operand::Imm(v) => pseudo::Operand::Imm(*v),
            Operand::Label(s) => pseudo::Operand::Label(s.clone()),
            Operand::MemOff(v, r) => pseudo::Operand::MemOff(*v, *r),
        })
        .collect()
}

fn resolve_pseudo_ops(
    ops: &[pseudo::Operand],
    pc: u32,
    sym: &SymbolTable,
) -> Result<Vec<pseudo::Operand>> {
    ops.iter()
        .map(|o| {
            if let pseudo::Operand::Label(s) = o {
                if let Some(lbl) = s.strip_prefix("%hi(").and_then(|s| s.strip_suffix(')')) {
                    let addr = resolve_label(lbl, sym)?;
                    return Ok(pseudo::Operand::Imm(
                        ((addr.wrapping_add(0x800)) >> 12) as i32,
                    ));
                }
                if let Some(lbl) = s.strip_prefix("%lo(").and_then(|s| s.strip_suffix(')')) {
                    let addr = resolve_label(lbl, sym)?;
                    let upper = (addr.wrapping_add(0x800)) >> 12;
                    return Ok(pseudo::Operand::Imm(addr as i32 - ((upper as i32) << 12)));
                }
                // CSR names must not be resolved as labels — leave them for csr_op.
                if is_csr_name(s) {
                    return Ok(o.clone());
                }
                let addr = resolve_label(s, sym)?;
                Ok(pseudo::Operand::Imm((addr as i32).wrapping_sub(pc as i32)))
            } else {
                Ok(o.clone())
            }
        })
        .collect()
}

fn is_csr_name(s: &str) -> bool {
    matches!(
        s,
        "fflags"
            | "frm"
            | "fcsr"
            | "cycle"
            | "time"
            | "instret"
            | "cycleh"
            | "instreth"
            | "mstatus"
            | "misa"
            | "mie"
            | "mtvec"
            | "mscratch"
            | "mepc"
            | "mcause"
            | "mtval"
            | "mip"
    )
}

fn resolve_ops(ops: &[Operand], pc: u32, sym: &SymbolTable) -> Result<Vec<pseudo::Operand>> {
    ops.iter()
        .map(|o| match o {
            Operand::Reg(r) => Ok(pseudo::Operand::Reg(*r)),
            Operand::FpReg(r) => Ok(pseudo::Operand::FpReg(*r)),
            Operand::Imm(v) => Ok(pseudo::Operand::Imm(*v)),
            Operand::MemOff(v, r) => Ok(pseudo::Operand::MemOff(*v, *r)),
            Operand::Label(s) => {
                if let Some(lbl) = s.strip_prefix("%hi(").and_then(|s| s.strip_suffix(')')) {
                    let addr = resolve_label(lbl, sym)?;
                    return Ok(pseudo::Operand::Imm(
                        ((addr.wrapping_add(0x800)) >> 12) as i32,
                    ));
                }
                if let Some(lbl) = s.strip_prefix("%lo(").and_then(|s| s.strip_suffix(')')) {
                    let addr = resolve_label(lbl, sym)?;
                    let upper = (addr.wrapping_add(0x800)) >> 12;
                    return Ok(pseudo::Operand::Imm(addr as i32 - ((upper as i32) << 12)));
                }
                let addr = resolve_label(s, sym)?;
                Ok(pseudo::Operand::Imm((addr as i32).wrapping_sub(pc as i32)))
            }
        })
        .collect()
}

fn resolve_label(name: &str, sym: &SymbolTable) -> Result<u32> {
    sym.resolve(name)
        .ok_or_else(|| anyhow!("undefined label: {name}"))
}

// ─── Real instruction encoder ─────────────────────────────────────────────────

fn encode_real(
    mnemonic: &str,
    ops: &[pseudo::Operand],
    _pc: u32,
    _sym: &SymbolTable,
) -> Result<u32> {
    use pseudo::Operand::*;

    // Helpers — closures borrow ops + mnemonic
    let reg = |i: usize| -> Result<u32> {
        match &ops[i] {
            Reg(r) => Ok(*r as u32),
            _ => Err(anyhow!(
                "{mnemonic}: op[{i}] expected int register, got {:?}",
                &ops[i]
            )),
        }
    };
    let fpreg = |i: usize| -> Result<u32> {
        match &ops[i] {
            FpReg(r) => Ok(*r as u32),
            Reg(r) => Ok(*r as u32), // allow xN in FP position (e.g. fmv.w.x)
            _ => Err(anyhow!("{mnemonic}: op[{i}] expected FP register")),
        }
    };
    let imm = |i: usize| -> Result<i32> {
        match &ops[i] {
            Imm(v) | MemOff(v, _) => Ok(*v),
            _ => Err(anyhow!("{mnemonic}: op[{i}] expected immediate")),
        }
    };
    // base register (from MemOff or plain Reg)
    let base = |i: usize| -> Result<u32> {
        match &ops[i] {
            MemOff(_, r) => Ok(*r as u32),
            _ => reg(i),
        }
    };

    // FP store S-type with opcode 0x27
    let enc_fp_s = |f3: u32, rs1: u32, rs2: u32, offset: i32| -> u32 {
        let i = (offset as u32) & 0xFFF;
        ((i >> 5) << 25) | (rs2 << 20) | (rs1 << 15) | (f3 << 12) | ((i & 0x1F) << 7) | 0x27
    };

    // FP arithmetic: opcode 0x53
    let enc_fparith = |f5: u32, fmt: u32, rm: u32, rd: u32, rs1: u32, rs2: u32| -> u32 {
        (f5 << 27) | (fmt << 25) | (rs2 << 20) | (rs1 << 15) | (rm << 12) | (rd << 7) | 0x53
    };

    // R4-type (FMADD etc.): bits 31:27 = rs3, bits 26:25 = fmt
    let enc_r4 = |opc: u32, fmt: u32, rd: u32, rs1: u32, rs2: u32, rs3: u32| -> u32 {
        (rs3 << 27) | (fmt << 25) | (rs2 << 20) | (rs1 << 15) | (7 << 12) | (rd << 7) | opc
    };

    // CSR name → address
    let csr_addr = |s: &str| -> u32 {
        match s {
            "fflags" => 0x001,
            "frm" => 0x002,
            "fcsr" => 0x003,
            "cycle" => 0xC00,
            "time" => 0xC01,
            "instret" => 0xC02,
            "cycleh" => 0xC80,
            "instreth" => 0xC82,
            "mstatus" => 0x300,
            "misa" => 0x301,
            "mie" => 0x304,
            "mtvec" => 0x305,
            "mscratch" => 0x340,
            "mepc" => 0x341,
            "mcause" => 0x342,
            "mtval" => 0x343,
            "mip" => 0x344,
            _ => u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0),
        }
    };
    let csr_op = |i: usize| -> u32 {
        match &ops[i] {
            Label(s) => csr_addr(s),
            Imm(v) => *v as u32,
            _ => 0,
        }
    };

    Ok(match mnemonic {
        // ── Integer R-type ────────────────────────────────────────────────────
        "add" => f::enc_r(0x33, 0x0, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "sub" => f::enc_r(0x33, 0x0, 0x20, reg(0)?, reg(1)?, reg(2)?),
        "sll" => f::enc_r(0x33, 0x1, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "slt" => f::enc_r(0x33, 0x2, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "sltu" => f::enc_r(0x33, 0x3, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "xor" => f::enc_r(0x33, 0x4, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "srl" => f::enc_r(0x33, 0x5, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "sra" => f::enc_r(0x33, 0x5, 0x20, reg(0)?, reg(1)?, reg(2)?),
        "or" => f::enc_r(0x33, 0x6, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "and" => f::enc_r(0x33, 0x7, 0x00, reg(0)?, reg(1)?, reg(2)?),

        // ── Integer I-type arithmetic ─────────────────────────────────────────
        "addi" => f::enc_i(0x13, 0x0, reg(0)?, reg(1)?, imm(2)?),
        "slti" => f::enc_i(0x13, 0x2, reg(0)?, reg(1)?, imm(2)?),
        "sltiu" => f::enc_i(0x13, 0x3, reg(0)?, reg(1)?, imm(2)?),
        "xori" => f::enc_i(0x13, 0x4, reg(0)?, reg(1)?, imm(2)?),
        "ori" => f::enc_i(0x13, 0x6, reg(0)?, reg(1)?, imm(2)?),
        "andi" => f::enc_i(0x13, 0x7, reg(0)?, reg(1)?, imm(2)?),
        "slli" => f::enc_i(0x13, 0x1, reg(0)?, reg(1)?, imm(2)? & 0x3F),
        "srli" => f::enc_i(0x13, 0x5, reg(0)?, reg(1)?, imm(2)? & 0x3F),
        "srai" => f::enc_i(0x13, 0x5, reg(0)?, reg(1)?, (imm(2)? & 0x3F) | (0x20 << 5)),

        // ── Integer loads ─────────────────────────────────────────────────────
        "lb" => f::enc_i(0x03, 0x0, reg(0)?, base(1)?, imm(1)?),
        "lh" => f::enc_i(0x03, 0x1, reg(0)?, base(1)?, imm(1)?),
        "lw" => f::enc_i(0x03, 0x2, reg(0)?, base(1)?, imm(1)?),
        "lbu" => f::enc_i(0x03, 0x4, reg(0)?, base(1)?, imm(1)?),
        "lhu" => f::enc_i(0x03, 0x5, reg(0)?, base(1)?, imm(1)?),
        // RV64I loads
        "ld" => f::enc_i(0x03, 0x3, reg(0)?, base(1)?, imm(1)?),
        "lwu" => f::enc_i(0x03, 0x6, reg(0)?, base(1)?, imm(1)?),

        // ── Integer stores ────────────────────────────────────────────────────
        "sb" => f::enc_s(0x0, base(1)?, reg(0)?, imm(1)?),
        "sh" => f::enc_s(0x1, base(1)?, reg(0)?, imm(1)?),
        "sw" => f::enc_s(0x2, base(1)?, reg(0)?, imm(1)?),
        // RV64I store
        "sd" => f::enc_s(0x3, base(1)?, reg(0)?, imm(1)?),

        // ── RV64I W-suffix I-type (opcode 0x1B) ──────────────────────────────
        "addiw" => f::enc_i(0x1B, 0x0, reg(0)?, reg(1)?, imm(2)?),
        "slliw" => f::enc_i(0x1B, 0x1, reg(0)?, reg(1)?, imm(2)? & 0x1F),
        "srliw" => f::enc_i(0x1B, 0x5, reg(0)?, reg(1)?, imm(2)? & 0x1F),
        "sraiw" => f::enc_i(0x1B, 0x5, reg(0)?, reg(1)?, (imm(2)? & 0x1F) | (0x20 << 5)),

        // ── RV64I W-suffix R-type (opcode 0x3B) ──────────────────────────────
        "addw" => f::enc_r(0x3B, 0x0, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "subw" => f::enc_r(0x3B, 0x0, 0x20, reg(0)?, reg(1)?, reg(2)?),
        "sllw" => f::enc_r(0x3B, 0x1, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "srlw" => f::enc_r(0x3B, 0x5, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "sraw" => f::enc_r(0x3B, 0x5, 0x20, reg(0)?, reg(1)?, reg(2)?),

        // ── Branches ──────────────────────────────────────────────────────────
        "beq" => f::enc_b(0x0, reg(0)?, reg(1)?, imm(2)?),
        "bne" => f::enc_b(0x1, reg(0)?, reg(1)?, imm(2)?),
        "blt" => f::enc_b(0x4, reg(0)?, reg(1)?, imm(2)?),
        "bge" => f::enc_b(0x5, reg(0)?, reg(1)?, imm(2)?),
        "bltu" => f::enc_b(0x6, reg(0)?, reg(1)?, imm(2)?),
        "bgeu" => f::enc_b(0x7, reg(0)?, reg(1)?, imm(2)?),

        // ── Jumps / upper imm ─────────────────────────────────────────────────
        "jal" => f::enc_j(reg(0)?, imm(1)?),
        "jalr" => f::enc_i(0x67, 0x0, reg(0)?, base(1)?, imm(1)?),
        "lui" => f::enc_u(0x37, reg(0)?, (imm(1)? as u32) << 12),
        "auipc" => f::enc_u(0x17, reg(0)?, (imm(1)? as u32) << 12),

        // ── System ────────────────────────────────────────────────────────────
        "ecall" => 0x0000_0073,
        "ebreak" => 0x0010_0073,
        "nop" => f::enc_i(0x13, 0, 0, 0, 0),

        // ── RV32M multiply/divide ─────────────────────────────────────────────
        "mul" | "mulh" | "mulhsu" | "mulhu" | "div" | "divu" | "rem" | "remu" => {
            rv32m::encode(mnemonic, reg(0)?, reg(1)?, reg(2)?)
                .ok_or_else(|| anyhow!("M-ext encode failed: {mnemonic}"))?
        }

        // ── CSR instructions ──────────────────────────────────────────────────
        // csrrw  rd, csr, rs1
        "csrrw" => f::enc_i(0x73, 0x1, reg(0)?, reg(2)?, csr_op(1) as i32),
        "csrrs" => f::enc_i(0x73, 0x2, reg(0)?, reg(2)?, csr_op(1) as i32),
        "csrrc" => f::enc_i(0x73, 0x3, reg(0)?, reg(2)?, csr_op(1) as i32),
        // csrrwi rd, csr, uimm5
        "csrrwi" => f::enc_i(0x73, 0x5, reg(0)?, imm(2)? as u32, csr_op(1) as i32),
        "csrrsi" => f::enc_i(0x73, 0x6, reg(0)?, imm(2)? as u32, csr_op(1) as i32),
        "csrrci" => f::enc_i(0x73, 0x7, reg(0)?, imm(2)? as u32, csr_op(1) as i32),

        // ── FP loads / stores ─────────────────────────────────────────────────
        "flw" => f::enc_i(0x07, 0x2, fpreg(0)?, base(1)?, imm(1)?),
        "fld" => f::enc_i(0x07, 0x3, fpreg(0)?, base(1)?, imm(1)?),
        "fsw" => enc_fp_s(0x2, base(1)?, fpreg(0)?, imm(1)?),
        "fsd" => enc_fp_s(0x3, base(1)?, fpreg(0)?, imm(1)?),

        // ── FMADD family (R4-type) ────────────────────────────────────────────
        "fmadd.s" => enc_r4(0x43, 0, fpreg(0)?, fpreg(1)?, fpreg(2)?, fpreg(3)?),
        "fmsub.s" => enc_r4(0x47, 0, fpreg(0)?, fpreg(1)?, fpreg(2)?, fpreg(3)?),
        "fnmsub.s" => enc_r4(0x4B, 0, fpreg(0)?, fpreg(1)?, fpreg(2)?, fpreg(3)?),
        "fnmadd.s" => enc_r4(0x4F, 0, fpreg(0)?, fpreg(1)?, fpreg(2)?, fpreg(3)?),
        "fmadd.d" => enc_r4(0x43, 1, fpreg(0)?, fpreg(1)?, fpreg(2)?, fpreg(3)?),
        "fmsub.d" => enc_r4(0x47, 1, fpreg(0)?, fpreg(1)?, fpreg(2)?, fpreg(3)?),
        "fnmsub.d" => enc_r4(0x4B, 1, fpreg(0)?, fpreg(1)?, fpreg(2)?, fpreg(3)?),
        "fnmadd.d" => enc_r4(0x4F, 1, fpreg(0)?, fpreg(1)?, fpreg(2)?, fpreg(3)?),

        // ── Single-precision arithmetic (opcode 0x53) ─────────────────────────
        "fadd.s" => enc_fparith(0x00, 0, 7, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fsub.s" => enc_fparith(0x01, 0, 7, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fmul.s" => enc_fparith(0x02, 0, 7, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fdiv.s" => enc_fparith(0x03, 0, 7, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fsqrt.s" => enc_fparith(0x0B, 0, 7, fpreg(0)?, fpreg(1)?, 0),
        "fsgnj.s" => enc_fparith(0x04, 0, 0, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fsgnjn.s" => enc_fparith(0x04, 0, 1, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fsgnjx.s" => enc_fparith(0x04, 0, 2, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fmin.s" => enc_fparith(0x05, 0, 0, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fmax.s" => enc_fparith(0x05, 0, 1, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fle.s" => enc_fparith(0x14, 0, 0, reg(0)?, fpreg(1)?, fpreg(2)?),
        "flt.s" => enc_fparith(0x14, 0, 1, reg(0)?, fpreg(1)?, fpreg(2)?),
        "feq.s" => enc_fparith(0x14, 0, 2, reg(0)?, fpreg(1)?, fpreg(2)?),
        "fcvt.w.s" => enc_fparith(0x18, 0, 7, reg(0)?, fpreg(1)?, 0),
        "fcvt.wu.s" => enc_fparith(0x18, 0, 7, reg(0)?, fpreg(1)?, 1),
        "fcvt.s.w" => enc_fparith(0x1A, 0, 7, fpreg(0)?, reg(1)?, 0),
        "fcvt.s.wu" => enc_fparith(0x1A, 0, 7, fpreg(0)?, reg(1)?, 1),
        "fmv.x.w" => enc_fparith(0x1C, 0, 0, reg(0)?, fpreg(1)?, 0),
        "fclass.s" => enc_fparith(0x1C, 0, 1, reg(0)?, fpreg(1)?, 0),
        "fmv.w.x" => enc_fparith(0x1E, 0, 0, fpreg(0)?, reg(1)?, 0),

        // ── Double-precision arithmetic ───────────────────────────────────────
        "fadd.d" => enc_fparith(0x00, 1, 7, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fsub.d" => enc_fparith(0x01, 1, 7, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fmul.d" => enc_fparith(0x02, 1, 7, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fdiv.d" => enc_fparith(0x03, 1, 7, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fsqrt.d" => enc_fparith(0x0B, 1, 7, fpreg(0)?, fpreg(1)?, 0),
        "fsgnj.d" => enc_fparith(0x04, 1, 0, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fsgnjn.d" => enc_fparith(0x04, 1, 1, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fsgnjx.d" => enc_fparith(0x04, 1, 2, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fmin.d" => enc_fparith(0x05, 1, 0, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fmax.d" => enc_fparith(0x05, 1, 1, fpreg(0)?, fpreg(1)?, fpreg(2)?),
        "fle.d" => enc_fparith(0x14, 1, 0, reg(0)?, fpreg(1)?, fpreg(2)?),
        "flt.d" => enc_fparith(0x14, 1, 1, reg(0)?, fpreg(1)?, fpreg(2)?),
        "feq.d" => enc_fparith(0x14, 1, 2, reg(0)?, fpreg(1)?, fpreg(2)?),
        "fcvt.w.d" => enc_fparith(0x18, 1, 7, reg(0)?, fpreg(1)?, 0),
        "fcvt.wu.d" => enc_fparith(0x18, 1, 7, reg(0)?, fpreg(1)?, 1),
        "fcvt.d.w" => enc_fparith(0x1A, 1, 7, fpreg(0)?, reg(1)?, 0),
        "fcvt.d.wu" => enc_fparith(0x1A, 1, 7, fpreg(0)?, reg(1)?, 1),
        "fclass.d" => enc_fparith(0x10, 1, 1, reg(0)?, fpreg(1)?, 0),
        "fcvt.s.d" => enc_fparith(0x20, 1, 7, fpreg(0)?, fpreg(1)?, 0),
        "fcvt.d.s" => enc_fparith(0x21, 0, 7, fpreg(0)?, fpreg(1)?, 0),

        other => return Err(anyhow!("unknown mnemonic: {other}")),
    })
}
