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
}

impl Memory {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            heap_ptr: HEAP_BASE,
            journal: HashMap::new(),
            journaling: false,
        }
    }

    /// Start recording writes for one instruction's worth of undo data.
    pub fn begin_journal(&mut self) {
        self.journal.clear();
        self.journaling = true;
    }

    /// Stop recording and return the undo list plus a snapshot of heap_ptr.
    pub fn end_journal(&mut self) -> (Vec<(u32, Option<u8>)>, u32) {
        self.journaling = false;
        (self.journal.drain().collect(), self.heap_ptr)
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
