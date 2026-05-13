# newton_sqrt.s — Newton-Raphson square root using RV32D (double precision)
#
# Demonstrates:
#   fcvt.d.s  fd, fs        — promote single → double
#   fcvt.d.w  fd, rs        — convert signed int → double
#   fadd.d    fd, fs1, fs2  — double add
#   fsub.d    fd, fs1, fs2  — double subtract
#   fmul.d    fd, fs1, fs2  — double multiply
#   fdiv.d    fd, fs1, fs2  — double divide
#   fabs.d    fd, fs        — absolute value (pseudo: fsgnjx.d fd, fs, fs)
#   flt.d     rd, fs1, fs2  — compare (fs1 < fs2) → integer register
#   fsqrt.d   fd, fs        — HW square root (compare against our result)
#
# Newton-Raphson iteration for sqrt(x):
#   x0 = x / 2   (initial guess)
#   x_{n+1} = (x_n + x / x_n) / 2
#   stop when |x_{n+1} - x_n| < epsilon
#
# We compute sqrt(2.0) and compare against the hardware fsqrt.d instruction.
# Expected:  ~1.41421356237

.data
two_d:  .double 2.0
eps_d:  .double 0.000001        # convergence threshold

msg_nr: .string "Newton-Raphson sqrt(2) = "
msg_hw: .string "Hardware   fsqrt.d(2) = "
newline:.string "\n"

.text
main:
    # ── load constants into fp registers ────────────────────────────────────
    la   t0, two_d
    fld  ft5, 0(t0)          # ft5 = 2.0 (double)

    la   t0, eps_d
    fld  ft6, 0(t0)          # ft6 = epsilon

    # half = 2.0 / 2.0 = 1.0 is messy to load — compute 0.5 from 1/2
    li   t0, 1
    fcvt.d.w ft4, t0         # ft4 = 1.0 (double)
    fdiv.d ft4, ft4, ft5     # ft4 = 0.5

    # ── initial guess x0 = 2.0 / 2.0 = 1.0 ─────────────────────────────────
    fmv.d fa0, ft4           # fa0 = 0.5  (will become our iterate x_n)
    # actually start at x0 = value/2
    fdiv.d fa0, ft5, ft5     # fa0 = 2.0/2.0 = 1.0

nr_loop:
    # x_{n+1} = (x_n + value/x_n) / 2
    fdiv.d ft0, ft5, fa0     # ft0 = value / x_n
    fadd.d ft0, fa0, ft0     # ft0 = x_n + value/x_n
    fmul.d ft0, ft0, ft4     # ft0 = (x_n + value/x_n) * 0.5

    # check convergence: |ft0 - fa0| < epsilon
    fsub.d ft1, ft0, fa0     # ft1 = x_{n+1} - x_n
    fabs.d ft1, ft1          # ft1 = |difference|
    flt.d  a0, ft1, ft6      # a0 = 1 if |diff| < epsilon
    bnez a0, nr_done

    fmv.d fa0, ft0           # x_n = x_{n+1}
    j    nr_loop

nr_done:
    fmv.d fa0, ft0           # result in fa0

    # print Newton-Raphson result (syscall 3 = print_double via fa0)
    la   a0, msg_nr
    li   a7, 4
    ecall
    li   a7, 3               # print_double  (fa0)
    ecall
    la   a0, newline
    li   a7, 4
    ecall

    # ── hardware sqrt for comparison ─────────────────────────────────────────
    la   a0, msg_hw
    li   a7, 4
    ecall

    fsqrt.d fa0, ft5         # fa0 = sqrt(2.0) via hardware
    li   a7, 3
    ecall
    la   a0, newline
    li   a7, 4
    ecall

    li   a7, 10
    ecall
