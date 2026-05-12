use std::collections::HashMap;

/// Well-known CSR addresses used by OARS.
#[allow(dead_code)]
pub mod addr {
    pub const FFLAGS: u32 = 0x001; // FP accrued exceptions
    pub const FRM: u32 = 0x002; // FP rounding mode
    pub const FCSR: u32 = 0x003; // FP control/status (fflags | frm<<5)
    pub const CYCLE: u32 = 0xC00;
    pub const TIME: u32 = 0xC01;
    pub const INSTRET: u32 = 0xC02;
    pub const CYCLEH: u32 = 0xC80;
    pub const INSTRETH: u32 = 0xC82;
    pub const MSTATUS: u32 = 0x300;
    pub const MISA: u32 = 0x301;
    pub const MIE: u32 = 0x304;
    pub const MTVEC: u32 = 0x305;
    pub const MSCRATCH: u32 = 0x340;
    pub const MEPC: u32 = 0x341;
    pub const MCAUSE: u32 = 0x342;
    pub const MTVAL: u32 = 0x343;
    pub const MIP: u32 = 0x344;
}

pub struct CsrFile {
    regs: HashMap<u32, u32>,
}

impl CsrFile {
    pub fn new() -> Self {
        let mut regs = HashMap::new();
        // MISA: MXL=1 (RV32), extensions I M F D
        regs.insert(
            addr::MISA,
            (1 << 30) | (1 << 8) | (1 << 12) | (1 << 5) | (1 << 3),
        );
        Self { regs }
    }

    pub fn read(&self, csr: u32) -> u32 {
        *self.regs.get(&csr).unwrap_or(&0)
    }

    pub fn write(&mut self, csr: u32, val: u32) {
        // Keep fcsr/fflags/frm in sync
        match csr {
            addr::FCSR => {
                self.regs.insert(addr::FFLAGS, val & 0x1F);
                self.regs.insert(addr::FRM, (val >> 5) & 0x7);
            }
            addr::FFLAGS => {
                let old_frm = self.read(addr::FRM);
                self.regs.insert(addr::FCSR, (val & 0x1F) | (old_frm << 5));
            }
            addr::FRM => {
                let old_flags = self.read(addr::FFLAGS);
                self.regs.insert(addr::FCSR, old_flags | ((val & 0x7) << 5));
            }
            _ => {}
        }
        self.regs.insert(csr, val);
    }

    pub fn set_bits(&mut self, csr: u32, mask: u32) {
        let v = self.read(csr) | mask;
        self.write(csr, v);
    }

    pub fn clear_bits(&mut self, csr: u32, mask: u32) {
        let v = self.read(csr) & !mask;
        self.write(csr, v);
    }

    /// Update cycle/instret performance counters.
    pub fn tick(&mut self, instret: u64) {
        self.regs.insert(addr::CYCLE, instret as u32);
        self.regs.insert(addr::CYCLEH, (instret >> 32) as u32);
        self.regs.insert(addr::INSTRET, instret as u32);
        self.regs.insert(addr::INSTRETH, (instret >> 32) as u32);
        self.regs.insert(addr::TIME, instret as u32);
    }
}

impl Default for CsrFile {
    fn default() -> Self {
        Self::new()
    }
}
