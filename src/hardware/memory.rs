use crate::util::error::OarsError;
use std::collections::HashMap;

pub const TEXT_BASE: u32 = 0x0040_0000;
pub const DATA_BASE: u32 = 0x1001_0000;
pub const HEAP_BASE: u32 = 0x1004_0000;
pub const STACK_TOP: u32 = 0x7FFF_EFFC;

/// Byte-addressed sparse memory. Unmapped addresses read as 0.
pub struct Memory {
    data: HashMap<u32, u8>,
    heap_ptr: u32,
    // Write journal for backstep: addr → value before first write this step.
    journal: HashMap<u32, Option<u8>>,
    journaling: bool,
    // heap_ptr captured at begin_journal so restore gets the pre-step value.
    journal_heap_ptr: u32,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            heap_ptr: HEAP_BASE,
            journal: HashMap::new(),
            journaling: false,
            journal_heap_ptr: HEAP_BASE,
        }
    }

    /// Start recording writes for one instruction's worth of undo data.
    pub fn begin_journal(&mut self) {
        self.journal.clear();
        self.journal_heap_ptr = self.heap_ptr; // snapshot BEFORE the step
        self.journaling = true;
    }

    /// Stop recording; returns (undo_bytes, pre_step_heap_ptr).
    pub fn end_journal(&mut self) -> (Vec<(u32, Option<u8>)>, u32) {
        self.journaling = false;
        (self.journal.drain().collect(), self.journal_heap_ptr)
    }

    /// Restore bytes written during a step (from a backstepper snapshot).
    pub fn restore_mem_undo(&mut self, undo: &[(u32, Option<u8>)], heap_ptr: u32) {
        for (addr, old) in undo {
            match old {
                None => {
                    self.data.remove(addr);
                }
                Some(v) => {
                    self.data.insert(*addr, *v);
                }
            }
        }
        self.heap_ptr = heap_ptr;
    }

    pub fn load_byte(&self, addr: u32) -> u8 {
        *self.data.get(&addr).unwrap_or(&0)
    }

    pub fn load_halfword(&self, addr: u32) -> u16 {
        (self.load_byte(addr) as u16) | ((self.load_byte(addr + 1) as u16) << 8)
    }

    pub fn load_word(&self, addr: u32) -> u32 {
        (self.load_byte(addr) as u32)
            | ((self.load_byte(addr + 1) as u32) << 8)
            | ((self.load_byte(addr + 2) as u32) << 16)
            | ((self.load_byte(addr + 3) as u32) << 24)
    }

    pub fn store_byte(&mut self, addr: u32, val: u8) {
        if self.journaling && !self.journal.contains_key(&addr) {
            let old = self.data.get(&addr).copied();
            self.journal.insert(addr, old);
        }
        self.data.insert(addr, val);
    }

    pub fn store_halfword(&mut self, addr: u32, val: u16) {
        self.store_byte(addr, val as u8);
        self.store_byte(addr + 1, (val >> 8) as u8);
    }

    pub fn store_word(&mut self, addr: u32, val: u32) {
        self.store_byte(addr, val as u8);
        self.store_byte(addr + 1, (val >> 8) as u8);
        self.store_byte(addr + 2, (val >> 16) as u8);
        self.store_byte(addr + 3, (val >> 24) as u8);
    }

    pub fn write_bytes(&mut self, addr: u32, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            self.store_byte(addr + i as u32, b);
        }
    }

    /// Read a null-terminated string from memory.
    pub fn read_cstring(&self, addr: u32) -> String {
        let mut s = Vec::new();
        let mut a = addr;
        loop {
            let b = self.load_byte(a);
            if b == 0 {
                break;
            }
            s.push(b);
            a = a.wrapping_add(1);
        }
        String::from_utf8_lossy(&s).into_owned()
    }

    /// sbrk: grow heap by `size` bytes, return old heap pointer.
    pub fn sbrk(&mut self, size: u32) -> Result<u32, OarsError> {
        let ptr = self.heap_ptr;
        self.heap_ptr = self
            .heap_ptr
            .checked_add(size)
            .ok_or_else(|| OarsError::Runtime {
                pc: 0,
                msg: "heap overflow".into(),
            })?;
        Ok(ptr)
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_records_old_byte() {
        let mut m = Memory::new();
        m.store_byte(0x100, 0xAB);
        m.begin_journal();
        m.store_byte(0x100, 0xFF);
        let (undo, _) = m.end_journal();
        let entry = undo.iter().find(|(a, _)| *a == 0x100).unwrap();
        assert_eq!(entry.1, Some(0xAB));
    }

    #[test]
    fn journal_first_write_wins() {
        // Writing the same address twice should record only the original value.
        let mut m = Memory::new();
        m.store_byte(0x100, 0x11);
        m.begin_journal();
        m.store_byte(0x100, 0x22);
        m.store_byte(0x100, 0x33);
        let (undo, _) = m.end_journal();
        let entry = undo.iter().find(|(a, _)| *a == 0x100).unwrap();
        assert_eq!(entry.1, Some(0x11));
    }

    #[test]
    fn journal_new_addr_records_none() {
        let mut m = Memory::new();
        m.begin_journal();
        m.store_byte(0x200, 0x42);
        let (undo, _) = m.end_journal();
        let entry = undo.iter().find(|(a, _)| *a == 0x200).unwrap();
        assert_eq!(entry.1, None);
    }

    #[test]
    fn restore_reverses_writes() {
        let mut m = Memory::new();
        m.store_byte(0x100, 0xAA);
        m.begin_journal();
        m.store_byte(0x100, 0xFF);
        m.store_byte(0x200, 0x42);
        let (undo, hp) = m.end_journal();

        m.restore_mem_undo(&undo, hp);

        assert_eq!(m.load_byte(0x100), 0xAA);
        assert_eq!(m.load_byte(0x200), 0x00); // was unmapped, back to 0
    }

    #[test]
    fn restore_reverses_sbrk() {
        let mut m = Memory::new();
        m.sbrk(64).unwrap();
        m.begin_journal();
        m.sbrk(128).unwrap();
        let (undo, hp) = m.end_journal();
        // hp should be the pre-step heap_ptr (HEAP_BASE + 64)
        assert_eq!(hp, HEAP_BASE + 64);

        m.restore_mem_undo(&undo, hp);
        // After restore, the next sbrk should start at HEAP_BASE+64 again.
        let next = m.sbrk(0).unwrap();
        assert_eq!(next, HEAP_BASE + 64);
    }

    #[test]
    fn store_word_round_trip() {
        let mut m = Memory::new();
        m.store_word(0x400, 0xDEAD_BEEF);
        assert_eq!(m.load_word(0x400), 0xDEAD_BEEF);
    }
}
