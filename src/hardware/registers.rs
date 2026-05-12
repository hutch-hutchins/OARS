pub const NUM_REGS: usize = 32;

pub const REG_NAMES: [&str; NUM_REGS] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2",
    "s0",   "s1", "a0", "a1", "a2", "a3", "a4", "a5",
    "a6",   "a7", "s2", "s3", "s4", "s5", "s6", "s7",
    "s8",   "s9", "s10","s11","t3", "t4", "t5", "t6",
];

pub struct RegisterFile {
    regs: [u32; NUM_REGS],
}

impl RegisterFile {
    pub fn new() -> Self {
        Self { regs: [0; NUM_REGS] }
    }

    #[inline]
    pub fn read(&self, index: usize) -> u32 {
        self.regs[index]
    }

    #[inline]
    pub fn write(&mut self, index: usize, value: u32) {
        if index != 0 {
            self.regs[index] = value;
        }
    }

    pub fn dump(&self) -> Vec<(String, u32)> {
        (0..NUM_REGS)
            .map(|i| (format!("x{:02}({})", i, REG_NAMES[i]), self.regs[i]))
            .collect()
    }

    pub fn snapshot(&self) -> [u32; NUM_REGS] { self.regs }
    pub fn restore(&mut self, snap: &[u32; NUM_REGS]) { self.regs = *snap; self.regs[0] = 0; }
}

impl Default for RegisterFile {
    fn default() -> Self { Self::new() }
}

/// Resolve a register name (ABI or xN) to its index (0–31).
pub fn parse_reg(name: &str) -> Option<usize> {
    if let Some(rest) = name.strip_prefix('x') {
        if let Ok(n) = rest.parse::<usize>() {
            if n < NUM_REGS { return Some(n); }
        }
    }
    Some(match name {
        "zero" => 0,  "ra" => 1,   "sp" => 2,  "gp" => 3,
        "tp"   => 4,  "t0" => 5,   "t1" => 6,  "t2" => 7,
        "s0" | "fp" => 8,           "s1" => 9,
        "a0"  => 10,  "a1" => 11,  "a2" => 12, "a3" => 13,
        "a4"  => 14,  "a5" => 15,  "a6" => 16, "a7" => 17,
        "s2"  => 18,  "s3" => 19,  "s4" => 20, "s5" => 21,
        "s6"  => 22,  "s7" => 23,  "s8" => 24, "s9" => 25,
        "s10" => 26,  "s11"=> 27,
        "t3"  => 28,  "t4" => 29,  "t5" => 30, "t6" => 31,
        _ => return None,
    })
}
