use crate::hardware::registers::{NUM_REGS, REG_NAMES};

pub struct RegFile64 {
    regs: [u64; NUM_REGS],
}

impl RegFile64 {
    pub fn new() -> Self {
        Self {
            regs: [0; NUM_REGS],
        }
    }

    #[inline]
    pub fn read(&self, index: usize) -> u64 {
        self.regs[index]
    }

    #[inline]
    pub fn write(&mut self, index: usize, value: u64) {
        if index != 0 {
            self.regs[index] = value;
        }
    }

    pub fn dump(&self) -> Vec<(String, u64)> {
        (0..NUM_REGS)
            .map(|i| (format!("x{:02}({})", i, REG_NAMES[i]), self.regs[i]))
            .collect()
    }

    pub fn snapshot(&self) -> [u64; NUM_REGS] {
        self.regs
    }

    pub fn restore(&mut self, snap: &[u64; NUM_REGS]) {
        self.regs = *snap;
        self.regs[0] = 0;
    }
}

impl Default for RegFile64 {
    fn default() -> Self {
        Self::new()
    }
}
