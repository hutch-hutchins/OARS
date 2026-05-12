/// Snapshot-based undo for single-step debugging.
///
/// Saves (pc, regs, fp_regs) before each instruction into a ring buffer.
/// Memory writes are NOT reversed — this is a simplification for Phase 2.

use crate::hardware::{fp_registers::FpRegisters, registers::RegisterFile};

const HISTORY_CAP: usize = 256;

#[derive(Clone)]
struct Snapshot {
    pc:   u32,
    regs: [u32; 32],
    fp:   [u64; 32],
}

pub struct Backstepper {
    buf:  Vec<Snapshot>,
    head: usize,   // index of the next slot to write
    len:  usize,   // number of valid entries
}

impl Backstepper {
    pub fn new() -> Self {
        Self { buf: Vec::with_capacity(HISTORY_CAP), head: 0, len: 0 }
    }

    pub fn push(&mut self, pc: u32, regs: &RegisterFile, fp: &FpRegisters) {
        let snap = Snapshot { pc, regs: regs.snapshot(), fp: fp.snapshot() };
        if self.buf.len() < HISTORY_CAP {
            self.buf.push(snap);
        } else {
            self.buf[self.head] = snap;
        }
        self.head = (self.head + 1) % HISTORY_CAP;
        self.len = (self.len + 1).min(HISTORY_CAP);
    }

    /// Restore the most recent snapshot, returning true if one was available.
    pub fn pop(&mut self, pc: &mut u32, regs: &mut RegisterFile, fp: &mut FpRegisters) -> bool {
        if self.len == 0 {
            return false;
        }
        self.len -= 1;
        self.head = (self.head + HISTORY_CAP - 1) % HISTORY_CAP;
        let snap = &self.buf[self.head];
        *pc = snap.pc;
        regs.restore(&snap.regs);
        fp.restore(&snap.fp);
        true
    }

    pub fn len(&self) -> usize { self.len }
}
