# integer_power.s — Exponentiation by squaring using RV32M  mul  and  mulh
#
# Demonstrates:
#   mul    rd, rs1, rs2   — lower 32 bits of rs1 * rs2 (RV32M)
#   mulh   rd, rs1, rs2   — upper 32 bits of signed rs1 * rs2 (RV32M)
#   mulhu  rd, rs1, rs2   — upper 32 bits of unsigned rs1 * rs2 (RV32M)
#
# Computes base^exp using the fast "exponentiation by squaring" method:
#   result = 1
#   while exp > 0:
#     if exp is odd: result *= base
#     base *= base
#     exp >>= 1
#
# Part 1:  2^10 = 1024  (fits in 32 bits — use  mul)
# Part 2:  shows mulhu to detect 64-bit overflow

.data
msg_a:  .string "2^10 = "
msg_b:  .string "\n12345 * 6789 upper half = "
newline:.string "\n"

.text
main:
    # ── Part 1: compute 2^10 ────────────────────────────────────────────────
    la   a0, msg_a
    li   a7, 4
    ecall

    li   t0, 2           # base = 2
    li   t1, 10          # exp  = 10
    li   t2, 1           # result = 1

pow_loop:
    beqz t1, pow_done

    andi t3, t1, 1       # if exp is odd...
    beqz t3, pow_skip
    mul  t2, t2, t0      # result *= base   (RV32M)

pow_skip:
    mul  t0, t0, t0      # base *= base     (RV32M)
    srli t1, t1, 1       # exp >>= 1
    j    pow_loop

pow_done:
    mv   a0, t2
    li   a7, 1
    ecall

    la   a0, newline
    li   a7, 4
    ecall

    # ── Part 2: mulhu — detect overflow in unsigned multiply ────────────────
    # 12345 * 6789 = 83,810,205  — fits in 32 bits, so mulhu gives 0
    la   a0, msg_b
    li   a7, 4
    ecall

    li   t0, 12345
    li   t1, 6789
    mulhu a0, t0, t1     # upper 32 bits of 12345 * 6789  (should be 0)
    li   a7, 1
    ecall

    la   a0, newline
    li   a7, 4
    ecall

    li   a0, 0
    li   a7, 10
    ecall
