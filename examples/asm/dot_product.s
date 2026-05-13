# dot_product.s — Vector dot product using fmadd.s (fused multiply-add)
#
# Demonstrates:
#   fmadd.s fd, fs1, fs2, fs3   — fd = fs1*fs2 + fs3  (single-precision FMA)
#   flw     fd, offset(rs)      — load float from memory
#   fadd.s  fd, fs1, fs2        — floating-point add
#   fcvt.s.w fd, rs             — int → float conversion
#
# fmadd.s performs fd = (fs1 * fs2) + fs3 in a single instruction.
# This is both faster and more accurate than separate mul + add because
# the intermediate product is not rounded before the addition.
#
# Computes:  u · v  where
#   u = [1.0, 2.0, 3.0, 4.0]
#   v = [4.0, 3.0, 2.0, 1.0]
#
# Expected:  1*4 + 2*3 + 3*2 + 4*1 = 4 + 6 + 6 + 4 = 20.0

.data
vec_u:  .float 1.0
        .float 2.0
        .float 3.0
        .float 4.0

vec_v:  .float 4.0
        .float 3.0
        .float 2.0
        .float 1.0

msg:    .string "u · v = "
newline:.string "\n"

.text
main:
    la   a0, msg
    li   a7, 4
    ecall

    la   t0, vec_u       # t0 = pointer into u
    la   t1, vec_v       # t1 = pointer into v
    li   t2, 4           # loop counter = 4 elements

    # accumulator starts at 0.0
    li   t3, 0
    fcvt.s.w fa0, t3     # fa0 = 0.0  (accumulator)

dot_loop:
    beqz t2, dot_done

    flw  ft0, 0(t0)      # ft0 = u[i]
    flw  ft1, 0(t1)      # ft1 = v[i]

    fmadd.s fa0, ft0, ft1, fa0   # fa0 = ft0*ft1 + fa0  (FMA)

    addi t0, t0, 4       # advance u pointer
    addi t1, t1, 4       # advance v pointer
    addi t2, t2, -1
    j    dot_loop

dot_done:
    li   a7, 2           # print_float (fa0)
    ecall

    la   a0, newline
    li   a7, 4
    ecall

    li   a7, 10
    ecall
