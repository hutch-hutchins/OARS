# big_mul64.s — 64-bit multiplication that overflows 32-bit arithmetic
#
# Computes 1,000,000 × 1,000,000 = 1,000,000,000,000 (10^12).
# This product exceeds the 32-bit unsigned range (2^32 ≈ 4.29 × 10^9).
# In RV64I, MUL produces the full 64-bit result without overflow.
#
# Then divides by 1,000,000,000 (10^9) to recover 1,000, proving the
# 64-bit intermediate value was correct.
#
# Expected output: 1000

        .text
main:
        li      t0, 1000000             # 10^6
        mul     t1, t0, t0              # t1 = 10^12  (64-bit MUL, no overflow)

        li      t2, 1000000000          # 10^9  (loaded via lui + addi by assembler)
        div     t3, t1, t2              # t3 = 10^12 / 10^9 = 1000  (64-bit DIV)

        mv      a0, t3
        li      a7, 1                   # print_int(1000)
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        li      a0, 0
        li      a7, 10                  # exit(0)
        ecall
