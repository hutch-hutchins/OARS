# sum_cubes64.s — Sum of cubes verified against the closed-form identity
#
# Identity:  1^3 + 2^3 + ... + N^3  =  [N×(N+1)/2]^2
#
# N = 1000:
#   Loop sum = 1 + 8 + 27 + ... + 10^9  =  250,500,250,000
#   Formula  = [1000×1001/2]^2 = 500500^2 = 250,500,250,000
#
# 250,500,250,000 > 2^37, far beyond 32-bit range (2^32 ≈ 4.3 × 10^9).
# 32-bit arithmetic would overflow around n = 362 and give a wrong total.
# All accumulation must use 64-bit registers.
#
# Prints "MATCH" if the loop matches the formula, proving 64-bit correctness.
#
# Expected output:
#   MATCH

        .text
main:
        # ── Loop: accumulate n^3 for n = 1..1000 ────────────────────────────
        li      t0, 0           # sum = 0  (64-bit)
        li      t1, 1           # n = 1
        li      t2, 1000        # limit
cubeloop:
        bgt     t1, t2, cube_done
        mul     t3, t1, t1      # n^2
        mul     t3, t3, t1      # n^3
        add     t0, t0, t3      # sum += n^3  (64-bit ADD)
        addi    t1, t1, 1
        j       cubeloop
cube_done:
        # t0 = 250,500,250,000

        # ── Formula: [N*(N+1)/2]^2 ────────────────────────────────────────────
        li      t1, 1000        # N
        li      t2, 1001        # N+1
        mul     t3, t1, t2      # 1000*1001 = 1,001,000
        srli    t3, t3, 1       # / 2  = 500,500
        mul     t4, t3, t3      # 500500^2 = 250,500,250,000  (64-bit MUL)

        # ── Compare ───────────────────────────────────────────────────────────
        bne     t0, t4, fail

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
