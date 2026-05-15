# fibonacci_big64.s — Fibonacci terms that genuinely exceed 32-bit range
#
# F(50) = 12,586,269,025 > 2^32 = 4,294,967,296
# A 32-bit register would overflow at F(47) = 2,971,215,073.
#
# Because the result is too large for print_int directly, we split it:
#   F(50) / 1,000,000,000 = 12
#   F(50) % 1,000,000,000 = 586,269,025
# and print each part on its own line.
#
# Expected output:
#   12
#   586269025

        .text
main:
        li      t0, 0           # a = F(0)
        li      t1, 1           # b = F(1)
        li      t2, 49          # iterate 49 times to reach F(50)
iter:
        beq     t2, zero, done
        add     t4, t0, t1      # next = a + b  (64-bit ADD — overflows 32-bit above F(47))
        mv      t0, t1
        mv      t1, t4
        addi    t2, t2, -1
        j       iter
done:
        # t1 = F(50) = 12,586,269,025
        li      t2, 1000000000          # 10^9

        div     a0, t1, t2              # quotient = 12
        li      a7, 1
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        rem     a0, t1, t2              # remainder = 586,269,025
        li      a7, 1
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        li      a0, 0
        li      a7, 10
        ecall
