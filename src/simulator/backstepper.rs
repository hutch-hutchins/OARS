/// Snapshot-based undo for single-step debugging.
///
/// Saves (pc, regs, fp_regs, mem_undo, heap_ptr) before each instruction into
/// a ring buffer. Memory writes are fully reversed on backstep.
use crate::hardware::{fp_registers::FpRegisters, memory::Memory, registers::RegisterFile};

const HISTORY_CAP: usize = 256;

#[derive(Clone)]
struct Snapshot {
    pc: u32,
    regs: [u32; 32],
    fp: [u64; 32],
    mem_undo: Vec<(u32, Option<u8>)>,
    heap_ptr: u32,
}

pub struct Backstepper {
    buf: Vec<Snapshot>,
    head: usize,
    len: usize,
}

impl Backstepper {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(HISTORY_CAP),
            head: 0,
            len: 0,
        }
    }

    /// Push a pre-step snapshot. `mem_undo` is the list of (addr, old_byte)
    /// returned by `Memory::end_journal()` after the step executed.
    pub fn push(
        &mut self,
        pc: u32,
        regs: [u32; 32],
        fp: [u64; 32],
        mem_undo: Vec<(u32, Option<u8>)>,
        heap_ptr: u32,
    ) {
        let snap = Snapshot {
            pc,
            regs,
            fp,
            mem_undo,
            heap_ptr,
        };
        if self.buf.len() < HISTORY_CAP {
            self.buf.push(snap);
        } else {
            self.buf[self.head] = snap;
        }
        self.head = (self.head + 1) % HISTORY_CAP;
        self.len = (self.len + 1).min(HISTORY_CAP);
    }

    /// Restore the most recent snapshot, returning true if one was available.
    pub fn pop(
        &mut self,
        pc: &mut u32,
        regs: &mut RegisterFile,
        fp: &mut FpRegisters,
        mem: &mut Memory,
    ) -> bool {
        if self.len == 0 {
            return false;
        }
        self.len -= 1;
        self.head = (self.head + HISTORY_CAP - 1) % HISTORY_CAP;
        let snap = self.buf[self.head].clone();
        *pc = snap.pc;
        regs.restore(&snap.regs);
        fp.restore(&snap.fp);
        mem.restore_mem_undo(&snap.mem_undo, snap.heap_ptr);
        true
    }

    pub fn len(&self) -> usize {
        self.len
    }
}
