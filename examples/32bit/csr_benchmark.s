# csr_benchmark.s — Instruction-count benchmarking with Zicsr
#
# Demonstrates:
#   csrr  rd, instret   — read "instructions retired" counter (pseudo for csrrs rd, instret, x0)
#   csrr  rd, cycle     — read clock cycle counter
#   sub   rd, rs1, rs2  — compute elapsed count
#   divu  rd, rs1, rs2  — unsigned divide (RV32M) to compute average
#
# The instret CSR increments by 1 for every instruction that retires.
# Reading it before and after a loop tells you exactly how many instructions
# the loop executed — useful for understanding code efficiency.
#
# This program benchmarks two ways to sum 1..N:
#   Method A — naive loop:     sum = 0; for i in 1..=N: sum += i
#   Method B — Gauss formula:  sum = N*(N+1)/2  (just 3 instructions)

.data
N:       .word 100

msg_a:   .string "Loop sum    = "
msg_b:   .string "Formula sum = "
msg_ic_a:.string "  (loop instrs: "
msg_ic_b:.string "  (formula instrs: "
msg_end: .string ")\n"
newline: .string "\n"

.text
main:
    la   t0, N
    lw   s0, 0(t0)       # s0 = N = 100

    # ════════════════════════════════════════════════════════════════════════
    # Method A: naive loop
    # ════════════════════════════════════════════════════════════════════════
    csrr s1, instret     # s1 = instret before loop

    li   t0, 0           # sum = 0
    li   t1, 1           # i = 1
loop_a:
    bgt  t1, s0, done_a  # if i > N, exit
    add  t0, t0, t1      # sum += i
    addi t1, t1, 1       # i++
    j    loop_a
done_a:
    csrr s2, instret     # s2 = instret after loop
    sub  s2, s2, s1      # s2 = instruction count for the loop
    mv   s3, t0          # save sum_a

    # print result
    la   a0, msg_a
    li   a7, 4
    ecall
    mv   a0, s3
    li   a7, 1
    ecall
    la   a0, msg_ic_a
    li   a7, 4
    ecall
    mv   a0, s2
    li   a7, 1
    ecall
    la   a0, msg_end
    li   a7, 4
    ecall

    # ════════════════════════════════════════════════════════════════════════
    # Method B: Gauss formula  N*(N+1)/2
    # ════════════════════════════════════════════════════════════════════════
    csrr s1, instret     # s1 = instret before formula

    addi t0, s0, 1       # t0 = N+1
    mul  t0, s0, t0      # t0 = N*(N+1)       (RV32M)
    srli t0, t0, 1       # t0 = N*(N+1)/2  (same as divu by 2)

    csrr s2, instret     # s2 = instret after formula
    sub  s2, s2, s1      # s2 = instruction count for the formula

    # print result
    la   a0, msg_b
    li   a7, 4
    ecall
    mv   a0, t0
    li   a7, 1
    ecall
    la   a0, msg_ic_b
    li   a7, 4
    ecall
    mv   a0, s2
    li   a7, 1
    ecall
    la   a0, msg_end
    li   a7, 4
    ecall

    li   a0, 0
    li   a7, 10
    ecall
