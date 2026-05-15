# math_lib.s — Reusable math subroutines
#
# Subroutines provided:
#   gcd        a0=x, a1=y  →  a0 = gcd(x, y)   (clobbers a1, t0)
#   int_power  a0=base, a1=exp  →  a0 = base^exp  (clobbers t0..t2)
#
# Usage:  .include "math_lib.s"
#
# No .text or .data directive here — the caller controls segment placement.
# All subroutines follow the RISC-V ABI: ra saved by caller if needed.

# ── gcd(a0, a1) → a0 ─────────────────────────────────────────────────────────
# Euclidean algorithm: gcd(a, 0) = a;  gcd(a, b) = gcd(b, a mod b)

gcd:
        beqz    a1, gcd_done
        rem     t0, a0, a1
        mv      a0, a1
        mv      a1, t0
        j       gcd
gcd_done:
        ret

# ── int_power(a0=base, a1=exp) → a0 ──────────────────────────────────────────
# Exponentiation by squaring — O(log exp) multiplications.
# base^0 = 1 for any base.

int_power:
        li      t0, 1           # result = 1
        mv      t1, a0          # t1 = base (working copy)
        mv      t2, a1          # t2 = exp
ip_loop:
        beqz    t2, ip_done
        andi    t3, t2, 1       # if exp is odd …
        beqz    t3, ip_even
        mul     t0, t0, t1      # result *= base
ip_even:
        mul     t1, t1, t1      # base = base^2
        srli    t2, t2, 1       # exp >>= 1
        j       ip_loop
ip_done:
        mv      a0, t0
        ret
