# quadratic.s — Quadratic formula using RV32F (single-precision)
#
# Demonstrates:
#   flw     fd, offset(rs)   — load float from memory
#   fcvt.s.w fd, rs          — convert signed int → float
#   fmul.s  fd, fs1, fs2     — multiply
#   fadd.s  fd, fs1, fs2     — add
#   fsub.s  fd, fs1, fs2     — subtract
#   fdiv.s  fd, fs1, fs2     — divide
#   fsqrt.s fd, fs           — square root
#   flt.s   rd, fs1, fs2     — compare (fs1 < fs2), result in integer reg
#   fmv.s   fd, fs           — copy float register (pseudo)
#   fneg.s  fd, fs           — negate (pseudo)
#
# Solves:  x^2 - 5x + 6 = 0   →   a=1, b=-5, c=6
# Expected roots: x = 3.0 and x = 2.0
#
# discriminant = b*b - 4*a*c
# x1 = (-b + sqrt(discriminant)) / (2*a)
# x2 = (-b - sqrt(discriminant)) / (2*a)

.data
coeff_a: .float 1.0
coeff_b: .float -5.0
coeff_c: .float 6.0
four:    .float 4.0
two:     .float 2.0

msg_eq:  .string "x^2 - 5x + 6 = 0\n"
msg_x1:  .string "x1 = "
msg_x2:  .string "x2 = "
msg_neg: .string "No real roots (discriminant < 0)\n"
newline: .string "\n"

.text
main:
    la   a0, msg_eq
    li   a7, 4
    ecall

    # ── load coefficients ────────────────────────────────────────────────────
    la   t0, coeff_a
    flw  fa0, 0(t0)      # fa0 = a

    la   t0, coeff_b
    flw  fa1, 0(t0)      # fa1 = b

    la   t0, coeff_c
    flw  fa2, 0(t0)      # fa2 = c

    la   t0, four
    flw  ft3, 0(t0)      # ft3 = 4.0

    la   t0, two
    flw  ft4, 0(t0)      # ft4 = 2.0

    # ── discriminant = b*b - 4*a*c ───────────────────────────────────────────
    fmul.s ft0, fa1, fa1  # ft0 = b*b
    fmul.s ft1, ft3, fa0  # ft1 = 4*a
    fmul.s ft1, ft1, fa2  # ft1 = 4*a*c
    fsub.s ft0, ft0, ft1  # ft0 = discriminant

    # ── check for negative discriminant ─────────────────────────────────────
    # Use a float zero: convert integer 0 to float
    li   t0, 0
    fcvt.s.w ft5, t0     # ft5 = 0.0

    flt.s a0, ft0, ft5   # a0 = 1 if discriminant < 0.0
    bnez a0, no_real_roots

    # ── sqrt(discriminant) ───────────────────────────────────────────────────
    fsqrt.s ft1, ft0     # ft1 = sqrt(disc)

    # ── denominator = 2*a ────────────────────────────────────────────────────
    fmul.s ft2, ft4, fa0 # ft2 = 2*a

    # ── x1 = (-b + sqrt(disc)) / (2*a) ──────────────────────────────────────
    fneg.s ft3, fa1      # ft3 = -b
    fadd.s ft0, ft3, ft1 # ft0 = -b + sqrt(disc)
    fdiv.s fa0, ft0, ft2 # fa0 = x1

    la   a0, msg_x1
    li   a7, 4
    ecall

    li   a7, 2           # print_float (fa0)
    ecall

    la   a0, newline
    li   a7, 4
    ecall

    # ── x2 = (-b - sqrt(disc)) / (2*a) ──────────────────────────────────────
    fsub.s ft0, ft3, ft1  # ft0 = -b - sqrt(disc)
    fdiv.s fa0, ft0, ft2  # fa0 = x2

    la   a0, msg_x2
    li   a7, 4
    ecall

    li   a7, 2
    ecall

    la   a0, newline
    li   a7, 4
    ecall

    li   a7, 10
    ecall

no_real_roots:
    la   a0, msg_neg
    li   a7, 4
    ecall
    li   a7, 10
    ecall
