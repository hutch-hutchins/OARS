use crate::hardware::{memory::Memory, registers::RegisterFile};
use crate::util::error::OarsError;
use std::io::{self, BufRead, Write};

/// Dispatch a RISC-V ECALL based on a7 (syscall number).
/// Returns true if the program should continue, false if it should exit.
pub fn dispatch(
    regs: &mut RegisterFile,
    mem: &mut Memory,
    pc: u32,
    stdout: &mut dyn Write,
    stdin: &mut dyn BufRead,
) -> Result<bool, OarsError> {
    let num = regs.read(17); // a7

    match num {
        // ── I/O ──────────────────────────────────────────────────────────────
        1 => {
            // print_int: a0 = integer
            let v = regs.read(10) as i32;
            write!(stdout, "{v}").ok();
        }

        2 => {
            // print_float: a0 = float bits — Phase 2
            let bits = regs.read(10);
            write!(stdout, "{}", f32::from_bits(bits)).ok();
        }

        4 => {
            // print_string: a0 = address of null-terminated string
            let addr = regs.read(10);
            let s = mem.read_cstring(addr);
            write!(stdout, "{s}").ok();
        }

        5 => {
            // read_int → a0
            let _ = stdout.flush();
            let mut line = String::new();
            stdin.read_line(&mut line)
                .map_err(|e| OarsError::Syscall { number: num, msg: e.to_string() })?;
            let v: i32 = line.trim().parse().unwrap_or(0);
            regs.write(10, v as u32);
        }

        8 => {
            // read_string: a0 = buffer address, a1 = max length
            let _ = stdout.flush();
            let addr = regs.read(10);
            let max  = regs.read(11) as usize;
            let mut line = String::new();
            stdin.read_line(&mut line)
                .map_err(|e| OarsError::Syscall { number: num, msg: e.to_string() })?;
            let bytes = line.as_bytes();
            let n = bytes.len().min(max.saturating_sub(1));
            mem.write_bytes(addr, &bytes[..n]);
            mem.store_byte(addr + n as u32, 0);
        }

        10 => {
            // exit
            return Ok(false);
        }

        11 => {
            // print_char: a0 = ASCII value
            let c = (regs.read(10) & 0xFF) as u8;
            write!(stdout, "{}", c as char).ok();
        }

        12 => {
            // read_char → a0
            let _ = stdout.flush();
            let mut line = String::new();
            stdin.read_line(&mut line)
                .map_err(|e| OarsError::Syscall { number: num, msg: e.to_string() })?;
            let c = line.chars().next().unwrap_or('\0') as u32;
            regs.write(10, c);
        }

        17 => {
            // exit2: exit code in a0
            return Ok(false);
        }

        // ── Heap allocation ───────────────────────────────────────────────────
        9 => {
            // sbrk: a0 = number of bytes to allocate → a0 = base address
            let size = regs.read(10);
            let ptr = mem.sbrk(size)?;
            regs.write(10, ptr);
        }

        // ── Unknown ───────────────────────────────────────────────────────────
        _ => {
            return Err(OarsError::Syscall {
                number: num,
                msg: format!("unknown syscall {num} at PC {pc:#010x}"),
            });
        }
    }

    Ok(true)
}
