// ─── Decode helpers ───────────────────────────────────────────────────────────

#[inline] pub fn opcode(w: u32) -> u32  { w & 0x7F }
#[inline] pub fn rd(w: u32)     -> usize { ((w >> 7)  & 0x1F) as usize }
#[inline] pub fn funct3(w: u32) -> u32  { (w >> 12) & 0x7 }
#[inline] pub fn rs1(w: u32)    -> usize { ((w >> 15) & 0x1F) as usize }
#[inline] pub fn rs2(w: u32)    -> usize { ((w >> 20) & 0x1F) as usize }
#[inline] pub fn funct7(w: u32) -> u32  { w >> 25 }

/// Sign-extend a `width`-bit value to i32.
#[inline]
pub fn sext(val: u32, width: u32) -> i32 {
    let shift = 32 - width;
    ((val << shift) as i32) >> shift
}

/// I-type immediate [11:0], sign-extended.
#[inline] pub fn imm_i(w: u32) -> i32 { sext(w >> 20, 12) }

/// S-type immediate, sign-extended.
#[inline]
pub fn imm_s(w: u32) -> i32 {
    sext(((w >> 7) & 0x1F) | ((w >> 25) << 5), 12)
}

/// B-type immediate (byte offset), sign-extended.
#[inline]
pub fn imm_b(w: u32) -> i32 {
    let imm = ((w >> 8)  & 0xF)  << 1   // bits 4:1
            | ((w >> 25) & 0x3F) << 5   // bits 10:5
            | ((w >> 7)  & 0x1)  << 11  // bit  11
            | ((w >> 31) & 0x1)  << 12; // bit  12
    sext(imm, 13)
}

/// U-type immediate (already shifted to bits [31:12]).
#[inline] pub fn imm_u(w: u32) -> u32 { w & 0xFFFF_F000 }

/// J-type immediate (byte offset), sign-extended.
#[inline]
pub fn imm_j(w: u32) -> i32 {
    let imm = ((w >> 21) & 0x3FF) << 1  // bits 10:1
            | ((w >> 20) & 0x1)   << 11 // bit  11
            | ((w >> 12) & 0xFF)  << 12 // bits 19:12
            | ((w >> 31) & 0x1)   << 20;// bit  20
    sext(imm, 21)
}

// ─── Encode helpers ───────────────────────────────────────────────────────────

#[inline]
pub fn enc_r(opc: u32, f3: u32, f7: u32, rd: u32, rs1: u32, rs2: u32) -> u32 {
    (f7 << 25) | (rs2 << 20) | (rs1 << 15) | (f3 << 12) | (rd << 7) | opc
}

#[inline]
pub fn enc_i(opc: u32, f3: u32, rd: u32, rs1: u32, imm: i32) -> u32 {
    (((imm as u32) & 0xFFF) << 20) | (rs1 << 15) | (f3 << 12) | (rd << 7) | opc
}

#[inline]
pub fn enc_s(f3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let i = (imm as u32) & 0xFFF;
    ((i >> 5) << 25) | (rs2 << 20) | (rs1 << 15) | (f3 << 12) | ((i & 0x1F) << 7) | 0x23
}

#[inline]
pub fn enc_b(f3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let i = (imm as u32) & 0x1FFF;
    let imm12   = (i >> 12) & 1;
    let imm11   = (i >> 11) & 1;
    let imm10_5 = (i >>  5) & 0x3F;
    let imm4_1  = (i >>  1) & 0xF;
    (imm12 << 31) | (imm10_5 << 25) | (rs2 << 20) | (rs1 << 15)
        | (f3 << 12) | (imm4_1 << 8) | (imm11 << 7) | 0x63
}

#[inline]
pub fn enc_u(opc: u32, rd: u32, imm: u32) -> u32 {
    (imm & 0xFFFF_F000) | (rd << 7) | opc
}

#[inline]
pub fn enc_j(rd: u32, imm: i32) -> u32 {
    let i = (imm as u32) & 0x1F_FFFF;
    let imm20    = (i >> 20) & 1;
    let imm19_12 = (i >> 12) & 0xFF;
    let imm11    = (i >> 11) & 1;
    let imm10_1  = (i >>  1) & 0x3FF;
    (imm20 << 31) | (imm10_1 << 21) | (imm11 << 20) | (imm19_12 << 12) | (rd << 7) | 0x6F
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_r() {
        // ADD x1, x2, x3
        let w = enc_r(0x33, 0, 0, 1, 2, 3);
        assert_eq!(opcode(w), 0x33);
        assert_eq!(rd(w), 1);
        assert_eq!(rs1(w), 2);
        assert_eq!(rs2(w), 3);
        assert_eq!(funct3(w), 0);
        assert_eq!(funct7(w), 0);
    }

    #[test]
    fn roundtrip_i_positive() {
        // ADDI x1, x2, 100
        let w = enc_i(0x13, 0, 1, 2, 100);
        assert_eq!(imm_i(w), 100);
        assert_eq!(rd(w), 1);
        assert_eq!(rs1(w), 2);
    }

    #[test]
    fn roundtrip_i_negative() {
        // ADDI x1, x2, -1
        let w = enc_i(0x13, 0, 1, 2, -1);
        assert_eq!(imm_i(w), -1);
    }

    #[test]
    fn roundtrip_s() {
        // SW x2, -4(x1)
        let w = enc_s(2, 1, 2, -4);
        assert_eq!(imm_s(w), -4);
        assert_eq!(rs1(w), 1);
        assert_eq!(rs2(w), 2);
    }

    #[test]
    fn roundtrip_b() {
        // BEQ x1, x2, 8
        let w = enc_b(0, 1, 2, 8);
        assert_eq!(imm_b(w), 8);
    }

    #[test]
    fn roundtrip_b_negative() {
        let w = enc_b(0, 1, 2, -8);
        assert_eq!(imm_b(w), -8);
    }

    #[test]
    fn roundtrip_j() {
        // JAL x1, 1024
        let w = enc_j(1, 1024);
        assert_eq!(imm_j(w), 1024);
        assert_eq!(rd(w), 1);
    }

    #[test]
    fn roundtrip_j_negative() {
        let w = enc_j(0, -4);
        assert_eq!(imm_j(w), -4);
    }

    #[test]
    fn imm_u_preserved() {
        // LUI x1, 0x12345
        let w = enc_u(0x37, 1, 0x12345 << 12);
        assert_eq!(imm_u(w), 0x12345 << 12);
    }
}
