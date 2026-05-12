# hello.s — minimal "Hello, World!" for OARS integration tests
#
# RARS syscall numbers (same as OARS Phase 1 target):
#   a7 = 4  → print_string  (a0 = address of null-terminated string)
#   a7 = 10 → exit

        .data
msg:    .string "Hello, World!\n"

        .text
        .globl main
main:
        li      a7, 4           # syscall: print_string
        la      a0, msg
        ecall

        li      a7, 10          # syscall: exit
        ecall
