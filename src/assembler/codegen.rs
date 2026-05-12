use crate::assembler::parser::{DataItem, Instruction, Operand, Seg, Statement};
use crate::assembler::symbol_table::SymbolTable;
use crate::hardware::memory::{Memory, DATA_BASE, TEXT_BASE};
use crate::isa::formats as f;
use crate::isa::pseudo;
use anyhow::{anyhow, Result};

/// Output of assembly: memory image + symbol table + entry PC.
pub struct AssemblyOutput {
    pub symbols: SymbolTable,
    pub entry: u32,
}

/// Assemble a list of `Statement`s into `mem`.
/// Two-pass: pass 1 collects symbol addresses, pass 2 encodes instructions.
pub fn assemble(stmts: &[Statement], mem: &mut Memory) -> Result<AssemblyOutput> {
    let symbols = pass1(stmts);
    let entry = pass2(stmts, &symbols, mem)?;
    Ok(AssemblyOutput { symbols, entry })
}

// ─── Pass 1: collect labels ───────────────────────────────────────────────────

fn pass1(stmts: &[Statement]) -> SymbolTable {
    let mut sym = SymbolTable::new();
    let mut text_pc = TEXT_BASE;
    let mut data_pc = DATA_BASE;
    let mut seg = Seg::Text;

    for stmt in stmts {
        match stmt {
            Statement::Segment(s, _) => seg = s.clone(),

            Statement::Label(name, _) => {
                let addr = if seg == Seg::Text { text_pc } else { data_pc };
                sym.define(name, addr);
            }

            Statement::Instr(instr) => {
                text_pc += instr_size(instr) * 4;
            }

            Statement::Data(item, _) => {
                data_pc += data_item_size(item);
            }

            Statement::Globl(_) => {}
        }
    }

    sym
}

/// Size of an instruction in number of 4-byte words (1 or 2 for pseudo-ops).
fn instr_size(instr: &Instruction) -> u32 {
    let pop = match instr.mnemonic.as_str() {
        "li" => {
            if let Some(Operand::Imm(v)) = instr.ops.get(1) {
                if *v >= -2048 && *v <= 2047 { 1 } else { 2 }
            } else { 1 }
        }
        "la" => 2,
        m => pseudo::expand(m, &conv_ops(&instr.ops))
            .map(|v| v.len() as u32)
            .unwrap_or(1),
    };
    pop
}

fn data_item_size(item: &DataItem) -> u32 {
    match item {
        DataItem::Byte(_)         => 1,
        DataItem::Half(_)         => 2,
        DataItem::Word(_) | DataItem::Float(_) => 4,
        DataItem::String(s)       => s.len() as u32 + 1,
        DataItem::Ascii(s)        => s.len() as u32,
        DataItem::Space(n)        => *n,
        DataItem::Align(exp)      => 1u32 << exp, // upper bound; exact handled in pass2
    }
}

// ─── Pass 2: encode instructions + data ──────────────────────────────────────

fn pass2(stmts: &[Statement], sym: &SymbolTable, mem: &mut Memory) -> Result<u32> {
    let mut text_pc = TEXT_BASE;
    let mut data_pc = DATA_BASE;
    let mut seg = Seg::Text;
    let mut entry = TEXT_BASE;
    let mut entry_set = false;

    // If there's a `main` label, that's the entry point
    if let Some(addr) = sym.resolve("main") {
        entry = addr;
        entry_set = true;
    }

    for stmt in stmts {
        match stmt {
            Statement::Segment(s, _) => seg = s.clone(),

            Statement::Label(name, _) => {
                if !entry_set && (name == "main" || name == "_start") {
                    entry = if seg == Seg::Text { text_pc } else { data_pc };
                }
            }

            Statement::Instr(instr) => {
                let words = encode_instr(instr, text_pc, sym)?;
                for w in &words {
                    mem.store_word(text_pc, *w);
                    text_pc += 4;
                }
            }

            Statement::Data(item, _) => {
                data_pc = emit_data(item, data_pc, mem);
            }

            Statement::Globl(_) => {}
        }
    }

    Ok(entry)
}

fn emit_data(item: &DataItem, addr: u32, mem: &mut Memory) -> u32 {
    match item {
        DataItem::Byte(v) => { mem.store_byte(addr, *v as u8); addr + 1 }
        DataItem::Half(v) => { mem.store_halfword(addr, *v as u16); addr + 2 }
        DataItem::Word(v) => { mem.store_word(addr, *v as u32); addr + 4 }
        DataItem::Float(v) => { mem.store_word(addr, v.to_bits()); addr + 4 }
        DataItem::String(s) => {
            let bytes = s.as_bytes();
            mem.write_bytes(addr, bytes);
            mem.store_byte(addr + bytes.len() as u32, 0);
            addr + bytes.len() as u32 + 1
        }
        DataItem::Ascii(s) => {
            let bytes = s.as_bytes();
            mem.write_bytes(addr, bytes);
            addr + bytes.len() as u32
        }
        DataItem::Space(n) => addr + n,
        DataItem::Align(exp) => {
            let align = 1u32 << exp;
            (addr + align - 1) & !(align - 1)
        }
    }
}

// ─── Instruction encoder ──────────────────────────────────────────────────────

fn encode_instr(instr: &Instruction, pc: u32, sym: &SymbolTable) -> Result<Vec<u32>> {
    // Try pseudo-instruction expansion first
    let pop_ops = conv_ops(&instr.ops);
    if let Some(expanded) = pseudo::expand(&instr.mnemonic, &pop_ops) {
        let mut words = Vec::new();
        let mut cur_pc = pc;
        for ri in expanded {
            // Resolve any label operands that were embedded by pseudo.rs
            let ops = resolve_pseudo_ops(&ri.ops, cur_pc, sym)?;
            words.push(encode_real(ri.mnemonic, &ops, cur_pc, sym)?);
            cur_pc += 4;
        }
        return Ok(words);
    }
    // Real instruction
    let ops = resolve_ops(&instr.ops, pc, sym)?;
    Ok(vec![encode_real(&instr.mnemonic, &ops, pc, sym)?])
}

/// Convert parser::Operand to pseudo::Operand (same shape, different module).
fn conv_ops(ops: &[Operand]) -> Vec<pseudo::Operand> {
    ops.iter().map(|o| match o {
        Operand::Reg(r)       => pseudo::Operand::Reg(*r),
        Operand::Imm(v)       => pseudo::Operand::Imm(*v),
        Operand::Label(s)     => pseudo::Operand::Label(s.clone()),
        Operand::MemOff(v, r) => pseudo::Operand::MemOff(*v, *r),
    }).collect()
}

/// Resolve label operands in pseudo-instruction output to immediates.
fn resolve_pseudo_ops(
    ops: &[pseudo::Operand],
    pc: u32,
    sym: &SymbolTable,
) -> Result<Vec<pseudo::Operand>> {
    ops.iter().map(|o| {
        if let pseudo::Operand::Label(s) = o {
            if let Some(lbl) = s.strip_prefix("%hi(").and_then(|s| s.strip_suffix(')')) {
                let addr = resolve_label(lbl, sym)?;
                let upper = addr.wrapping_add(0x800) >> 12;
                return Ok(pseudo::Operand::Imm(upper as i32));
            }
            if let Some(lbl) = s.strip_prefix("%lo(").and_then(|s| s.strip_suffix(')')) {
                let addr = resolve_label(lbl, sym)?;
                let upper = addr.wrapping_add(0x800) >> 12;
                let lower = addr as i32 - ((upper as i32) << 12);
                return Ok(pseudo::Operand::Imm(lower));
            }
            // PC-relative label for branch/jal
            let addr = resolve_label(s, sym)?;
            let offset = (addr as i32).wrapping_sub(pc as i32);
            Ok(pseudo::Operand::Imm(offset))
        } else {
            Ok(o.clone())
        }
    }).collect()
}

fn resolve_ops(ops: &[Operand], pc: u32, sym: &SymbolTable) -> Result<Vec<pseudo::Operand>> {
    ops.iter().map(|o| match o {
        Operand::Reg(r)       => Ok(pseudo::Operand::Reg(*r)),
        Operand::Imm(v)       => Ok(pseudo::Operand::Imm(*v)),
        Operand::MemOff(v, r) => Ok(pseudo::Operand::MemOff(*v, *r)),
        Operand::Label(s)     => {
            // %hi/%lo modifiers
            if let Some(lbl) = s.strip_prefix("%hi(").and_then(|s| s.strip_suffix(')')) {
                let addr = resolve_label(lbl, sym)?;
                let upper = addr.wrapping_add(0x800) >> 12;
                return Ok(pseudo::Operand::Imm(upper as i32));
            }
            if let Some(lbl) = s.strip_prefix("%lo(").and_then(|s| s.strip_suffix(')')) {
                let addr = resolve_label(lbl, sym)?;
                let upper = addr.wrapping_add(0x800) >> 12;
                let lower = addr as i32 - ((upper as i32) << 12);
                return Ok(pseudo::Operand::Imm(lower));
            }
            let addr = resolve_label(s, sym)?;
            let offset = (addr as i32).wrapping_sub(pc as i32);
            Ok(pseudo::Operand::Imm(offset))
        }
    }).collect()
}

fn resolve_label(name: &str, sym: &SymbolTable) -> Result<u32> {
    sym.resolve(name).ok_or_else(|| anyhow!("undefined label: {name}"))
}

// ─── Real instruction encoder ─────────────────────────────────────────────────

fn encode_real(
    mnemonic: &str,
    ops: &[pseudo::Operand],
    _pc: u32,
    _sym: &SymbolTable,
) -> Result<u32> {
    use pseudo::Operand::*;

    let reg = |i: usize| -> Result<u32> {
        if let Reg(r) = &ops[i] { Ok(*r as u32) }
        else { Err(anyhow!("{mnemonic}: operand {i} expected register")) }
    };
    let imm = |i: usize| -> Result<i32> {
        match &ops[i] {
            Imm(v)       => Ok(*v),
            MemOff(v, _) => Ok(*v),
            _            => Err(anyhow!("{mnemonic}: operand {i} expected immediate")),
        }
    };
    let base = |i: usize| -> Result<u32> {
        if let MemOff(_, r) = &ops[i] { Ok(*r as u32) }
        else { reg(i) }
    };

    Ok(match mnemonic {
        // R-type
        "add"  => f::enc_r(0x33, 0x0, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "sub"  => f::enc_r(0x33, 0x0, 0x20, reg(0)?, reg(1)?, reg(2)?),
        "sll"  => f::enc_r(0x33, 0x1, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "slt"  => f::enc_r(0x33, 0x2, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "sltu" => f::enc_r(0x33, 0x3, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "xor"  => f::enc_r(0x33, 0x4, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "srl"  => f::enc_r(0x33, 0x5, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "sra"  => f::enc_r(0x33, 0x5, 0x20, reg(0)?, reg(1)?, reg(2)?),
        "or"   => f::enc_r(0x33, 0x6, 0x00, reg(0)?, reg(1)?, reg(2)?),
        "and"  => f::enc_r(0x33, 0x7, 0x00, reg(0)?, reg(1)?, reg(2)?),

        // I-type arithmetic
        "addi"  => f::enc_i(0x13, 0x0, reg(0)?, reg(1)?, imm(2)?),
        "slti"  => f::enc_i(0x13, 0x2, reg(0)?, reg(1)?, imm(2)?),
        "sltiu" => f::enc_i(0x13, 0x3, reg(0)?, reg(1)?, imm(2)?),
        "xori"  => f::enc_i(0x13, 0x4, reg(0)?, reg(1)?, imm(2)?),
        "ori"   => f::enc_i(0x13, 0x6, reg(0)?, reg(1)?, imm(2)?),
        "andi"  => f::enc_i(0x13, 0x7, reg(0)?, reg(1)?, imm(2)?),
        "slli"  => f::enc_i(0x13, 0x1, reg(0)?, reg(1)?, imm(2)? & 0x1F),
        "srli"  => f::enc_i(0x13, 0x5, reg(0)?, reg(1)?, imm(2)? & 0x1F),
        "srai"  => f::enc_i(0x13, 0x5, reg(0)?, reg(1)?, (imm(2)? & 0x1F) | (0x20 << 5)),

        // Loads
        "lb"  => f::enc_i(0x03, 0x0, reg(0)?, base(1)?, imm(1)?),
        "lh"  => f::enc_i(0x03, 0x1, reg(0)?, base(1)?, imm(1)?),
        "lw"  => f::enc_i(0x03, 0x2, reg(0)?, base(1)?, imm(1)?),
        "lbu" => f::enc_i(0x03, 0x4, reg(0)?, base(1)?, imm(1)?),
        "lhu" => f::enc_i(0x03, 0x5, reg(0)?, base(1)?, imm(1)?),

        // Stores
        "sb"  => f::enc_s(0x0, base(1)?, reg(0)?, imm(1)?),
        "sh"  => f::enc_s(0x1, base(1)?, reg(0)?, imm(1)?),
        "sw"  => f::enc_s(0x2, base(1)?, reg(0)?, imm(1)?),

        // Branches
        "beq"  => f::enc_b(0x0, reg(0)?, reg(1)?, imm(2)?),
        "bne"  => f::enc_b(0x1, reg(0)?, reg(1)?, imm(2)?),
        "blt"  => f::enc_b(0x4, reg(0)?, reg(1)?, imm(2)?),
        "bge"  => f::enc_b(0x5, reg(0)?, reg(1)?, imm(2)?),
        "bltu" => f::enc_b(0x6, reg(0)?, reg(1)?, imm(2)?),
        "bgeu" => f::enc_b(0x7, reg(0)?, reg(1)?, imm(2)?),

        // J-type
        "jal"  => f::enc_j(reg(0)?, imm(1)?),

        // JALR
        "jalr" => f::enc_i(0x67, 0x0, reg(0)?, base(1)?, imm(1)?),

        // Upper immediates
        "lui"   => f::enc_u(0x37, reg(0)?, (imm(1)? as u32) << 12),
        "auipc" => f::enc_u(0x17, reg(0)?, (imm(1)? as u32) << 12),

        // System
        "ecall"  => 0x0000_0073,
        "ebreak" => 0x0010_0073,

        // NOP
        "nop"    => f::enc_i(0x13, 0, 0, 0, 0),

        other => return Err(anyhow!("unknown mnemonic: {other}")),
    })
}
