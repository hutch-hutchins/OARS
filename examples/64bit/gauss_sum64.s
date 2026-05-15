# gauss_sum64.s — Sum 1..N where the result overflows 32-bit
#
# N = 100,000
# Sum = N × (N+1) / 2 = 100,000 × 100,001 / 2 = 5,000,050,000
#
# 5,000,050,000 > 2^32 (= 4,294,967,296), so a 32-bit accumulator would overflow.
# Both the loop accumulator and the Gauss formula must use 64-bit arithmetic.
#
# The program computes both and prints "MATCH" if they are equal, proving that
# the 64-bit additions and multiplications produced identical results.
#
# Expected output:
#   MATCH

        .text
main:
        # ── Loop sum ─────────────────────────────────────────────────────────
        li      t0, 0           # sum = 0  (64-bit accumulator)
        li      t1, 1           # i = 1
        li      t2, 100000      # N = 100,000
loop:
        bgt     t1, t2, loop_done
        add     t0, t0, t1      # sum += i  (64-bit ADD)
        addi    t1, t1, 1
        j       loop
loop_done:
        # t0 = 5,000,050,000  (> 2^32, proven 64-bit)

        # ── Gauss formula: N*(N+1)/2 ──────────────────────────────────────────
        li      t1, 100000      # N
        li      t2, 100001      # N+1
        mul     t3, t1, t2      # N*(N+1) = 10,000,100,000  (64-bit MUL)
        srli    t3, t3, 1       # / 2  = 5,000,050,000

        # ── Compare ───────────────────────────────────────────────────────────
        bne     t0, t3, fail

        la      a0, ok_msg
        li      a7, 4
        ecall
        j       done

fail:
        la      a0, fail_msg
        li      a7, 4
        ecall

done:
        li      a0, 0
        li      a7, 10
        ecall

        .data
ok_msg:   .string "MATCH\n"
fail_msg: .string "FAIL\n"
