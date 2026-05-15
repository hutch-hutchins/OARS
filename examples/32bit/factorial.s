# factorial.s — recursive factorial using the call stack
#
# Computes 10! = 3628800 and prints it.
# Demonstrates saving/restoring ra and a0 across recursive calls.

        .text
        .globl main

# int factorial(int n)  — a0 = n, returns n! in a0
factorial:
        addi    sp, sp, -8
        sw      ra, 4(sp)
        sw      a0, 0(sp)           # save n

        li      t0, 1
        ble     a0, t0, fact_base   # n <= 1 → return 1

        addi    a0, a0, -1
        call    factorial           # a0 = factorial(n-1)

        lw      t0, 0(sp)           # restore n
        mul     a0, a0, t0          # a0 = n * factorial(n-1)
        j       fact_restore

fact_base:
        li      a0, 1

fact_restore:
        lw      ra, 4(sp)
        addi    sp, sp, 8
        ret

main:
        li      a0, 10
        call    factorial           # a0 = 3628800

        mv      a1, a0
        la      a0, msg
        li      a7, 4
        ecall                       # print "10! = "

        mv      a0, a1
        li      a7, 1
        ecall                       # print 3628800

        la      a0, newline
        li      a7, 4
        ecall

        li      a0, 0
        li      a7, 10
        ecall

        .data
msg:    .asciz  "10! = "
newline:.asciz  "\n"
