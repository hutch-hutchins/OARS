# hello64.s — Hello World in RV64I mode
# Verifies the 64-bit engine handles basic string-printing syscalls identically
# to 32-bit mode.

        .text
main:
        li      a7, 4           # syscall 4 = print_string
        la      a0, msg
        ecall

        li      a0, 0
        li      a7, 10          # syscall 10 = exit(0)
        ecall

        .data
msg:    .string "Hello, 64-bit World!\n"
