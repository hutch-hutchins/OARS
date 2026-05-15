# word_ops64.s — Demonstrate RV64I W-suffix instructions
#
# ADDI  operates on the full 64-bit register; no 32-bit overflow possible.
# ADDIW operates on the lower 32 bits and sign-extends the 32-bit result to 64.
#
# Because LUI sign-extends in RV64I, INT32_MAX (0x7FFF_FFFF) is built via
# slli/addi rather than a plain li:
#   slli t0, t0, 31  shifts 1 left 31 places → 0x80000000 (2^31, 64-bit clean)
#   addi t0, t0, -1  → 0x7FFF_FFFF = INT32_MAX
#
# Starting from INT32_MAX (2,147,483,647):
#   addi  t1, t0, 1  → 2,147,483,648   (fits in 64-bit, NOT in signed 32-bit)
#   addiw t2, t0, 1  → -2,147,483,648  (32-bit overflow, sign-extended to 64)
#   addw  t4, t0, t0 → -2              (lower-32-bit sum truncated and sign-extended)
#   add   t5, t0, t0 → 4,294,967,294   (full 64-bit sum, no truncation)

        .text
main:
        # Build INT32_MAX in a way that works in 64-bit mode
        li      t0, 1
        slli    t0, t0, 31              # t0 = 0x80000000 = 2,147,483,648
        addi    t0, t0, -1              # t0 = 0x7FFF_FFFF = 2,147,483,647

        # 64-bit add: no overflow
        addi    t1, t0, 1               # t1 = 2,147,483,648

        # W-suffix add: 32-bit overflow → sign-extended INT32_MIN
        addiw   t2, t0, 1               # t2 = -2,147,483,648

        # Print t1 as unsigned (syscall 36) = "2147483648"
        mv      a0, t1
        li      a7, 36
        ecall
        li      a0, '\n'
        li      a7, 11
        ecall

        # Print t2 as signed (syscall 1) = "-2147483648"
        mv      a0, t2
        li      a7, 1
        ecall
        li      a0, '\n'
        li      a7, 11
        ecall

        # ADDW: lower-32-bit addition with sign extension
        addw    t4, t0, t0              # (INT32_MAX + INT32_MAX) truncated to 32-bit = -2
        mv      a0, t4
        li      a7, 1                   # print_int = "-2"
        ecall
        li      a0, '\n'
        li      a7, 11
        ecall

        # ADD (64-bit): same operands, no truncation = 4,294,967,294
        add     t5, t0, t0
        mv      a0, t5
        li      a7, 36                  # print_unsigned = "4294967294"
        ecall
        li      a0, '\n'
        li      a7, 11
        ecall

        li      a0, 0
        li      a7, 10                  # exit(0)
        ecall
