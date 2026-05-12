# bubble_sort.s — sort an array of 5 integers and print the result
#
# Array stored in .data; in-place bubble sort.
# Registers:
#   s0 = base address of array
#   s1 = outer loop counter i
#   s2 = inner loop counter j
#   t0, t1 = elements being compared
#   t2 = swap temp

        .data
arr:    .word 5, 3, 1, 4, 2
n:      .word 5

        .text
        .globl main
main:
        la      s0, arr
        la      t3, n
        lw      s3, 0(t3)       # s3 = n = 5

outer:
        li      s1, 0           # i = 0
inner_init:
        sub     t3, s3, s1      # limit = n - i
        addi    t3, t3, -1      # limit = n - i - 1
        li      s2, 0           # j = 0

inner:
        bge     s2, t3, outer_inc

        # load arr[j] and arr[j+1]
        slli    t4, s2, 2       # byte offset = j * 4
        add     t5, s0, t4
        lw      t0, 0(t5)       # t0 = arr[j]
        lw      t1, 4(t5)       # t1 = arr[j+1]

        ble     t0, t1, no_swap
        # swap
        sw      t1, 0(t5)
        sw      t0, 4(t5)
no_swap:
        addi    s2, s2, 1
        j       inner

outer_inc:
        addi    s1, s1, 1
        blt     s1, s3, inner_init

        # print sorted array
        li      s2, 0
print_loop:
        bge     s2, s3, exit
        slli    t4, s2, 2
        add     t5, s0, t4
        lw      a0, 0(t5)
        li      a7, 1           # print_int
        ecall
        li      a0, ' '
        li      a7, 11          # print_char
        ecall
        addi    s2, s2, 1
        j       print_loop

exit:
        li      a7, 10
        ecall
