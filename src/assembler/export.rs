use crate::hardware::memory::{Memory, DATA_BASE, TEXT_BASE};

// ─── Flat binary ──────────────────────────────────────────────────────────────

/// Raw little-endian machine code for the text segment, from TEXT_BASE to
/// `text_end`.  Load this at `TEXT_BASE` to run it directly.
pub fn flat_binary(mem: &Memory, text_end: u32) -> Vec<u8> {
    let len = text_end.saturating_sub(TEXT_BASE) as usize;
    mem.read_range(TEXT_BASE, len)
}

// ─── ELF32 ───────────────────────────────────────────────────────────────────

const EM_RISCV: u16 = 243;
const ET_EXEC: u16 = 2;
const EV_CURRENT: u32 = 1;
const PT_LOAD: u32 = 1;
const PF_X: u32 = 0x1;
const PF_W: u32 = 0x2;
const PF_R: u32 = 0x4;
const ELF_HEADER_SIZE: u32 = 52;
const PHDR_SIZE: u32 = 32;
const PAGE: u32 = 0x1000;

/// Produce a minimal ELF32 RISC-V executable.
///
/// The file contains:
///   - one PT_LOAD segment covering the text (r-x)
///   - one PT_LOAD segment covering the data (rw-), omitted if no data was assembled
pub fn elf32(mem: &Memory, entry: u32, text_end: u32, data_end: u32) -> Vec<u8> {
    let text_bytes = flat_binary(mem, text_end);
    let has_data = data_end > DATA_BASE;
    let data_bytes: Vec<u8> = if has_data {
        let len = (data_end - DATA_BASE) as usize;
        mem.read_range(DATA_BASE, len)
    } else {
        Vec::new()
    };

    let phnum: u16 = if has_data { 2 } else { 1 };
    let headers_size = ELF_HEADER_SIZE + PHDR_SIZE * phnum as u32;

    // File offsets for segment data
    let text_file_off = headers_size;
    let data_file_off = text_file_off + text_bytes.len() as u32;

    let mut buf = Vec::with_capacity(headers_size as usize + text_bytes.len() + data_bytes.len());

    // ── ELF header ────────────────────────────────────────────────────────────
    buf.extend_from_slice(&[0x7f, b'E', b'L', b'F']); // magic
    buf.push(1); // EI_CLASS  = ELFCLASS32
    buf.push(1); // EI_DATA   = ELFDATA2LSB
    buf.push(1); // EI_VERSION = EV_CURRENT
    buf.push(0); // EI_OSABI  = ELFOSABI_NONE
    buf.extend_from_slice(&[0u8; 8]); // padding
    w16(&mut buf, ET_EXEC);
    w16(&mut buf, EM_RISCV);
    w32(&mut buf, EV_CURRENT);
    w32(&mut buf, entry);
    w32(&mut buf, ELF_HEADER_SIZE); // e_phoff
    w32(&mut buf, 0); // e_shoff (no section headers)
    w32(&mut buf, 0x0004); // e_flags = EF_RISCV_FLOAT_ABI_DOUBLE
    w16(&mut buf, ELF_HEADER_SIZE as u16); // e_ehsize
    w16(&mut buf, PHDR_SIZE as u16); // e_phentsize
    w16(&mut buf, phnum); // e_phnum
    w16(&mut buf, 40); // e_shentsize (conventional even when shnum=0)
    w16(&mut buf, 0); // e_shnum
    w16(&mut buf, 0); // e_shstrndx

    // ── Program header — text (r-x) ───────────────────────────────────────────
    write_phdr(
        &mut buf,
        text_file_off,
        TEXT_BASE,
        text_bytes.len() as u32,
        text_bytes.len() as u32,
        PF_R | PF_X,
    );

    // ── Program header — data (rw-) ───────────────────────────────────────────
    if has_data {
        write_phdr(
            &mut buf,
            data_file_off,
            DATA_BASE,
            data_bytes.len() as u32,
            data_bytes.len() as u32,
            PF_R | PF_W,
        );
    }

    // ── Segment data ──────────────────────────────────────────────────────────
    buf.extend_from_slice(&text_bytes);
    buf.extend_from_slice(&data_bytes);

    buf
}

fn write_phdr(buf: &mut Vec<u8>, offset: u32, vaddr: u32, filesz: u32, memsz: u32, flags: u32) {
    w32(buf, PT_LOAD);
    w32(buf, offset);
    w32(buf, vaddr); // p_vaddr
    w32(buf, vaddr); // p_paddr
    w32(buf, filesz);
    w32(buf, memsz);
    w32(buf, flags);
    w32(buf, PAGE); // p_align
}

fn w16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn w32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
