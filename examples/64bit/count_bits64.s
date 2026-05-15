# count_bits64.s — Population count (Hamming weight) of a 64-bit value
#
# Counts the number of 1 bits using Brian Kernighan's algorithm:
#   while n != 0: n = n & (n-1); count++
# Each iteration clears the lowest set bit.
#
# The input value is  0x0000_0001_FFFF_FFFF  =  2^33 − 1  =  8,589,934,591
# Built as:  t0 = 4; slli t0, t0, 31  →  4 × 2^31 = 2^33 = 8,589,934,592
#            addi t0, t0, -1           →  2^33 − 1  =  8,589,934,591
#
# Bit layout:  upper 32 bits = 0x00000001 (1 set bit)
#              lower 32 bits = 0xFFFFFFFF (32 set bits)
#              total = 33 set bits
#
# A 32-bit popcount would see only 0xFFFFFFFF and return 32 (missing the 33rd bit).
#
# Expected output:
#   33

        .text
main:
        # Build 0x1FFFFFFFF = 2^33 - 1
        li      t0, 4
        slli    t0, t0, 31              # t0 = 4 × 2^31 = 2^33 = 8,589,934,592
        addi    t0, t0, -1             # t0 = 2^33 - 1 = 8,589,934,591 = 0x1FFFFFFFF

        li      t1, 0                   # count = 0
poploop:
        beq     t0, zero, popdone
        addi    t2, t0, -1             # t2 = n - 1
        and     t0, t0, t2              # n = n & (n-1)  clears lowest set bit (64-bit AND)
        addi    t1, t1, 1
        j       poploop
popdone:
        mv      a0, t1
        li      a7, 1                   # print 33
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        li      a0, 0
        li      a7, 10
        ecall
