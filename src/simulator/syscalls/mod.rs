use crate::hardware::{fp_registers::FpRegisters, memory::Memory, registers::RegisterFile};
use crate::util::error::OarsError;
use std::collections::VecDeque;
use std::io::{BufRead, Write};

/// Dispatch a RISC-V ECALL based on a7 (syscall number).
/// Returns true if the program should continue, false if it should exit.
pub fn dispatch(
    regs:   &mut RegisterFile,
    fp:     &mut FpRegisters,
    mem:    &mut Memory,
    pc:     u32,
    stdout: &mut dyn Write,
    stdin:  &mut dyn BufRead,
) -> Result<bool, OarsError> {
    let num = regs.read(17); // a7

    match num {
        // ── Integer / string I/O ──────────────────────────────────────────────
        1 => {
            // print_int: a0 = signed integer
            let v = regs.read(10) as i32;
            write!(stdout, "{v}").ok();
        }

        2 => {
            // print_float: fa0 (f10) = single-precision value
            let v = fp.read_f32(10);
            write!(stdout, "{v}").ok();
        }

        3 => {
            // print_double: fa0 (f10) = double-precision value
            let v = fp.read_f64(10);
            write!(stdout, "{v}").ok();
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

        6 => {
            // read_float → fa0 (f10)
            let _ = stdout.flush();
            let mut line = String::new();
            stdin.read_line(&mut line)
                .map_err(|e| OarsError::Syscall { number: num, msg: e.to_string() })?;
            let v: f32 = line.trim().parse().unwrap_or(0.0);
            fp.write_f32(10, v);
        }

        7 => {
            // read_double → fa0 (f10)
            let _ = stdout.flush();
            let mut line = String::new();
            stdin.read_line(&mut line)
                .map_err(|e| OarsError::Syscall { number: num, msg: e.to_string() })?;
            let v: f64 = line.trim().parse().unwrap_or(0.0);
            fp.write_f64(10, v);
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

        9 => {
            // sbrk: a0 = number of bytes → a0 = base address
            let size = regs.read(10);
            let ptr = mem.sbrk(size)?;
            regs.write(10, ptr);
        }

        10 | 17 => {
            // exit / exit2
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

        _ => {
            return Err(OarsError::Syscall {
                number: num,
                msg: format!("unknown syscall {num} at PC {pc:#010x}"),
            });
        }
    }

    Ok(true)
}

// ─── GUI variant ─────────────────────────────────────────────────────────────

pub enum GuiSyscallOutcome { Continue, Halt, NeedInput }

/// Dispatch an ECALL from the GUI run loop.
/// Output is appended to `console`. Input is consumed from `input_queue`;
/// if the queue is empty when a read syscall fires, returns `NeedInput`.
pub fn dispatch_gui(
    regs:        &mut RegisterFile,
    fp:          &mut FpRegisters,
    mem:         &mut Memory,
    _pc:         u32,
    console:     &mut String,
    input_queue: &mut VecDeque<String>,
) -> Result<GuiSyscallOutcome, OarsError> {
    let num = regs.read(17);

    match num {
        1 => {
            let v = regs.read(10) as i32;
            console.push_str(&v.to_string());
        }
        2 => { console.push_str(&fp.read_f32(10).to_string()); }
        3 => { console.push_str(&fp.read_f64(10).to_string()); }
        4 => {
            let addr = regs.read(10);
            console.push_str(&mem.read_cstring(addr));
        }
        5 => {
            if input_queue.is_empty() { return Ok(GuiSyscallOutcome::NeedInput); }
            let line = input_queue.pop_front().unwrap();
            regs.write(10, line.trim().parse::<i32>().unwrap_or(0) as u32);
        }
        6 => {
            if input_queue.is_empty() { return Ok(GuiSyscallOutcome::NeedInput); }
            let line = input_queue.pop_front().unwrap();
            fp.write_f32(10, line.trim().parse::<f32>().unwrap_or(0.0));
        }
        7 => {
            if input_queue.is_empty() { return Ok(GuiSyscallOutcome::NeedInput); }
            let line = input_queue.pop_front().unwrap();
            fp.write_f64(10, line.trim().parse::<f64>().unwrap_or(0.0));
        }
        8 => {
            if input_queue.is_empty() { return Ok(GuiSyscallOutcome::NeedInput); }
            let line = input_queue.pop_front().unwrap();
            let addr = regs.read(10);
            let max  = regs.read(11) as usize;
            let bytes = line.as_bytes();
            let n = bytes.len().min(max.saturating_sub(1));
            mem.write_bytes(addr, &bytes[..n]);
            mem.store_byte(addr + n as u32, 0);
        }
        9 => {
            let size = regs.read(10);
            let ptr = mem.sbrk(size)?;
            regs.write(10, ptr);
        }
        10 | 17 => return Ok(GuiSyscallOutcome::Halt),
        11 => {
            let c = (regs.read(10) & 0xFF) as u8;
            console.push(c as char);
        }
        12 => {
            if input_queue.is_empty() { return Ok(GuiSyscallOutcome::NeedInput); }
            let line = input_queue.pop_front().unwrap();
            let c = line.chars().next().unwrap_or('\0') as u32;
            regs.write(10, c);
        }
        _ => return Err(OarsError::Syscall {
            number: num,
            msg: format!("unknown syscall {num}"),
        }),
    }

    Ok(GuiSyscallOutcome::Continue)
}
