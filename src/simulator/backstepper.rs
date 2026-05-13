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

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for Backstepper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{fp_registers::FpRegisters, memory::Memory, registers::RegisterFile};

    fn make_cpu() -> (RegisterFile, FpRegisters, Memory) {
        (RegisterFile::new(), FpRegisters::new(), Memory::new())
    }

    #[test]
    fn pop_empty_returns_false() {
        let mut bs = Backstepper::new();
        let (mut regs, mut fp, mut mem) = make_cpu();
        let mut pc = 0u32;
        assert!(!bs.pop(&mut pc, &mut regs, &mut fp, &mut mem));
    }

    #[test]
    fn push_then_pop_restores_pc_and_regs() {
        let mut bs = Backstepper::new();
        let (mut regs, mut fp, mut mem) = make_cpu();

        let saved_pc = 0x0040_0004u32;
        let saved_regs = regs.snapshot();
        let saved_fp = fp.snapshot();
        bs.push(saved_pc, saved_regs, saved_fp, vec![], mem.end_journal().1);

        // Mutate state after the push
        regs.write(10, 0xDEAD);
        let mut pc = 0x0040_0008u32;

        let ok = bs.pop(&mut pc, &mut regs, &mut fp, &mut mem);
        assert!(ok);
        assert_eq!(pc, saved_pc);
        assert_eq!(regs.read(10), 0);
    }

    #[test]
    fn pop_restores_memory_write() {
        let mut bs = Backstepper::new();
        let (mut regs, mut fp, mut mem) = make_cpu();

        mem.begin_journal();
        mem.store_word(0x1001_0000, 0xCAFE_BABE);
        let (undo, hp) = mem.end_journal();
        bs.push(0x100, regs.snapshot(), fp.snapshot(), undo, hp);

        // Verify write happened
        assert_eq!(mem.load_word(0x1001_0000), 0xCAFE_BABE);

        let mut pc = 0u32;
        bs.pop(&mut pc, &mut regs, &mut fp, &mut mem);

        assert_eq!(mem.load_word(0x1001_0000), 0x0000_0000); // restored
    }

    #[test]
    fn ring_buffer_drops_oldest() {
        let mut bs = Backstepper::new();
        let (regs, fp, mut mem) = make_cpu();
        let hp = mem.end_journal().1;

        // Fill the ring buffer past capacity
        for i in 0..=(super::HISTORY_CAP as u32) {
            bs.push(i, regs.snapshot(), fp.snapshot(), vec![], hp);
        }
        assert_eq!(bs.len(), super::HISTORY_CAP);

        // The top snapshot should be the last one pushed (HISTORY_CAP)
        let (mut regs2, mut fp2, mut mem2) = make_cpu();
        let mut pc = 0u32;
        bs.pop(&mut pc, &mut regs2, &mut fp2, &mut mem2);
        assert_eq!(pc, super::HISTORY_CAP as u32);
    }
}
