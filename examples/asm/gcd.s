# gcd.s — Euclidean GCD using the RV32M  rem  instruction
#
# Demonstrates:
#   rem   rd, rs1, rs2   — signed remainder (RV32M)
#   beqz  rs, label      — branch if zero (pseudo: beq rs, x0, label)
#   mv    rd, rs         — register copy (pseudo: addi rd, rs, 0)
#
# Algorithm:  gcd(a, b):
#   while b != 0:
#     t = a rem b
#     a = b
#     b = t
#   return a
#
# Expected output:  GCD(48, 18) = 6

.data
msg1:   .string "GCD("
msg2:   .string ", "
msg3:   .string ") = "
newline:.string "\n"

.text
main:
    # ── print "GCD(48, 18) = " ──────────────────────────────────────────────
    la   a0, msg1
    li   a7, 4
    ecall

    li   a0, 48
    li   a7, 1
    ecall

    la   a0, msg2
    li   a7, 4
    ecall

    li   a0, 18
    li   a7, 1
    ecall

    la   a0, msg3
    li   a7, 4
    ecall

    # ── compute gcd(48, 18) ─────────────────────────────────────────────────
    li   a0, 48          # a = 48
    li   a1, 18          # b = 18

gcd_loop:
    beqz a1, gcd_done    # if b == 0, done

    rem  t0, a0, a1      # t = a rem b   (RV32M)
    mv   a0, a1          # a = b
    mv   a1, t0          # b = t
    j    gcd_loop

gcd_done:
    # a0 holds the result; print it
    li   a7, 1
    ecall

    la   a0, newline
    li   a7, 4
    ecall

    li   a7, 10
    ecall
