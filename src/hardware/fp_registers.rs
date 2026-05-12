pub const FP_REG_NAMES: [&str; 32] = [
    "ft0", "ft1", "ft2",  "ft3",  "ft4",  "ft5",  "ft6",  "ft7",
    "fs0", "fs1", "fa0",  "fa1",  "fa2",  "fa3",  "fa4",  "fa5",
    "fa6", "fa7", "fs2",  "fs3",  "fs4",  "fs5",  "fs6",  "fs7",
    "fs8", "fs9", "fs10", "fs11", "ft8",  "ft9",  "ft10", "ft11",
];

/// Unified 64-bit FP register file (NaN-boxed for single-precision values).
/// f0–f31 each hold 64 bits; single-precision values occupy the lower 32 bits
/// with upper 32 bits set to 0xFFFF_FFFF (canonical NaN-box per RISC-V spec).
pub struct FpRegisters {
    regs: [u64; 32],
}

impl FpRegisters {
    pub fn new() -> Self {
        // All zeros initially (interpreted as +0.0 in both f32 and f64)
        Self { regs: [0; 32] }
    }

    #[inline] pub fn read_f32(&self, i: usize)  -> f32  { f32::from_bits(self.regs[i] as u32) }
    #[inline] pub fn read_f64(&self, i: usize)  -> f64  { f64::from_bits(self.regs[i]) }
    #[inline] pub fn read_u32(&self, i: usize)  -> u32  { self.regs[i] as u32 }
    #[inline] pub fn read_u64(&self, i: usize)  -> u64  { self.regs[i] }

    #[inline]
    pub fn write_f32(&mut self, i: usize, v: f32) {
        // NaN-box: upper 32 bits = 0xFFFF_FFFF
        self.regs[i] = 0xFFFF_FFFF_0000_0000 | (v.to_bits() as u64);
    }

    #[inline] pub fn write_f64(&mut self, i: usize, v: f64)  { self.regs[i] = v.to_bits(); }
    #[inline] pub fn write_u32(&mut self, i: usize, v: u32)  { self.regs[i] = 0xFFFF_FFFF_0000_0000 | (v as u64); }
    #[inline] pub fn write_u64(&mut self, i: usize, v: u64)  { self.regs[i] = v; }

    pub fn dump(&self) -> Vec<(String, f64)> {
        (0..32)
            .map(|i| (format!("f{i:02}({})", FP_REG_NAMES[i]), f64::from_bits(self.regs[i])))
            .collect()
    }

    pub fn snapshot(&self) -> [u64; 32] { self.regs }
    pub fn restore(&mut self, snap: &[u64; 32]) { self.regs = *snap; }
}

impl Default for FpRegisters {
    fn default() -> Self { Self::new() }
}

/// Resolve an FP register name to its index (0–31).
pub fn parse_fp_reg(name: &str) -> Option<usize> {
    if let Some(rest) = name.strip_prefix('f') {
        // fN numeric names (f0–f31)
        if let Ok(n) = rest.parse::<usize>() {
            if n < 32 { return Some(n); }
        }
    }
    Some(match name {
        "ft0"  => 0,  "ft1"  => 1,  "ft2"  => 2,  "ft3"  => 3,
        "ft4"  => 4,  "ft5"  => 5,  "ft6"  => 6,  "ft7"  => 7,
        "fs0"  => 8,  "fs1"  => 9,
        "fa0"  => 10, "fa1"  => 11, "fa2"  => 12, "fa3"  => 13,
        "fa4"  => 14, "fa5"  => 15, "fa6"  => 16, "fa7"  => 17,
        "fs2"  => 18, "fs3"  => 19, "fs4"  => 20, "fs5"  => 21,
        "fs6"  => 22, "fs7"  => 23, "fs8"  => 24, "fs9"  => 25,
        "fs10" => 26, "fs11" => 27,
        "ft8"  => 28, "ft9"  => 29, "ft10" => 30, "ft11" => 31,
        _ => return None,
    })
}
