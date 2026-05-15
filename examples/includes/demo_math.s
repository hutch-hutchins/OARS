# demo_math.s — Demonstrates .include with a shared math library
#
# Includes math_lib.s which provides gcd and int_power subroutines.
#
# Computes and prints:
#   gcd(48, 18)   = 6
#   gcd(100, 75)  = 25
#   2^10          = 1024
#   3^5           = 243
#
# Expected output:
#   6
#   25
#   1024
#   243

        .text
        .include "math_lib.s"

main:
        # ── gcd(48, 18) ──────────────────────────────────────────────────────
        li      a0, 48
        li      a1, 18
        call    gcd
        li      a7, 1
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        # ── gcd(100, 75) ─────────────────────────────────────────────────────
        li      a0, 100
        li      a1, 75
        call    gcd
        li      a7, 1
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        # ── 2^10 ─────────────────────────────────────────────────────────────
        li      a0, 2
        li      a1, 10
        call    int_power
        li      a7, 1
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        # ── 3^5 ──────────────────────────────────────────────────────────────
        li      a0, 3
        li      a1, 5
        call    int_power
        li      a7, 1
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        li      a0, 0
        li      a7, 10
        ecall
