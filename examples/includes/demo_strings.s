# demo_strings.s — Demonstrates .include with a shared string library
#
# Includes string_lib.s which provides str_len and str_upper subroutines.
#
# 1. Measures the length of "hello, world!" → 13
# 2. Uppercases the string in place → "HELLO, WORLD!"
# 3. Prints the result
#
# Expected output:
#   13
#   HELLO, WORLD!

        .text
        .include "string_lib.s"

main:
        # ── strlen("hello, world!") ──────────────────────────────────────────
        la      a0, greeting
        call    str_len         # a0 = 13
        li      a7, 1
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        # ── str_upper in place, then print ───────────────────────────────────
        la      a0, greeting
        call    str_upper       # modifies greeting[] in memory

        la      a0, greeting
        li      a7, 4
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        li      a0, 0
        li      a7, 10
        ecall

        .data
greeting: .string "hello, world!"
