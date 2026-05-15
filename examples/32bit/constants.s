# constants.s — demonstrates .equ / .set symbolic constants
#
# Defines several named constants with .equ, then uses them as
# immediate operands in li, addi, and loop-bound instructions.
#
# Expected output:
#   SIZE = 8
#   SUM  = 36   (0+1+2+3+4+5+6+7+8)... wait, 0..SIZE-1 = 0..7 → sum 28
#   Actually: sum 1..=SIZE = 1+2+3+4+5+6+7+8 = 36

        .equ    SIZE, 8
        .equ    FIRST, 1

        .data
msg_size: .asciiz "SIZE = "
msg_sum:  .asciiz "\nSUM  = "
msg_nl:   .asciiz "\n"

        .text
        .globl main

main:
        # Print "SIZE = "
        la      a0, msg_size
        li      a7, 4
        ecall

        # Print the value of SIZE
        li      a0, SIZE
        li      a7, 1
        ecall

        # Print "\nSUM  = "
        la      a0, msg_sum
        li      a7, 4
        ecall

        # Compute sum = FIRST + (FIRST+1) + ... + SIZE  i.e. 1+2+...+8 = 36
        li      t0, FIRST           # t0 = i = FIRST
        li      t1, SIZE            # t1 = limit
        li      t2, 0               # t2 = sum

loop:
        bgt     t0, t1, done        # if i > SIZE: exit
        add     t2, t2, t0
        addi    t0, t0, 1
        j       loop

done:
        mv      a0, t2
        li      a7, 1
        ecall

        la      a0, msg_nl
        li      a7, 4
        ecall

        li      a0, 0
        li      a7, 10
        ecall
