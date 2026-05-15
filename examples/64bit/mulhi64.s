# mulhi64.s — High-half multiply (MULHU) with a product that overflows 64 bits
#
# a = 5,000,000,000  (5 × 10^9, larger than 2^32 = 4,294,967,296)
# a × a  = 25,000,000,000,000,000,000  (25 × 10^18)
# 2^64   = 18,446,744,073,709,551,616  (≈ 1.845 × 10^19)
#
# Because 25 × 10^18 > 2^64, the product does NOT fit in a single 64-bit register.
# MULHU returns the upper 64 bits of the full 128-bit unsigned product:
#
#   upper = floor(25 × 10^18 / 2^64) = 1
#
# A 32-bit MUL would give a completely wrong truncated result.
# MULH* instructions are the RV64 way to implement wide arithmetic.
#
# Expected output:
#   upper half = 1

        .text
main:
        li      t0, 5
        li      t1, 1000000000          # 10^9
        mul     t0, t0, t1              # t0 = 5 × 10^9  (64-bit MUL, > 2^32)

        mulhu   t1, t0, t0              # t1 = upper 64 bits of (5×10^9)²
                                        # (5×10^9)² = 25×10^18; 25×10^18/2^64 = 1.355 → t1 = 1

        la      a0, label
        li      a7, 4
        ecall                           # print "upper half = "

        mv      a0, t1
        li      a7, 1
        ecall                           # print 1

        li      a0, '\n'
        li      a7, 11
        ecall

        li      a0, 0
        li      a7, 10
        ecall

        .data
label:  .string "upper half = "
