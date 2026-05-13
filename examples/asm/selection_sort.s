# selection_sort.s — in-place selection sort on an integer array
#
# Sorts arr[] = {64, 25, 12, 22, 11} ascending using selection sort.
# Demonstrates nested loops, index arithmetic, and conditional swaps.
#
# Expected output: 11 12 22 25 64

        .data
arr:    .word   64, 25, 12, 22, 11
n:      .word   5

        .text
        .globl main

main:
        la      s0, arr             # s0 = base address
        la      t0, n
        lw      s1, 0(t0)           # s1 = n = 5

        # Outer loop: i = 0..n-1
        li      s2, 0               # s2 = i

outer:
        addi    t0, s1, -1
        bge     s2, t0, print       # i >= n-1: done sorting

        # Find index of minimum in arr[i..n-1]
        mv      s3, s2              # s3 = min_idx = i
        addi    s4, s2, 1           # s4 = j = i+1

inner:
        bge     s4, s1, swap        # j >= n: inner loop done

        slli    t0, s4, 2
        add     t0, s0, t0
        lw      t1, 0(t0)           # t1 = arr[j]

        slli    t2, s3, 2
        add     t2, s0, t2
        lw      t3, 0(t2)           # t3 = arr[min_idx]

        bge     t1, t3, inner_inc   # arr[j] >= arr[min_idx]: skip
        mv      s3, s4              # min_idx = j

inner_inc:
        addi    s4, s4, 1
        j       inner

        # Swap arr[i] and arr[min_idx] if min_idx != i
swap:
        beq     s3, s2, outer_inc  # no swap needed

        slli    t0, s2, 2
        add     t0, s0, t0
        lw      t1, 0(t0)           # t1 = arr[i]

        slli    t2, s3, 2
        add     t2, s0, t2
        lw      t3, 0(t2)           # t3 = arr[min_idx]

        sw      t3, 0(t0)           # arr[i] = arr[min_idx]
        sw      t1, 0(t2)           # arr[min_idx] = arr[i]

outer_inc:
        addi    s2, s2, 1
        j       outer

        # Print sorted array
print:
        li      s2, 0

print_loop:
        bge     s2, s1, done
        slli    t0, s2, 2
        add     t0, s0, t0
        lw      a0, 0(t0)
        li      a7, 1
        ecall                       # print integer

        li      a0, ' '
        li      a7, 11
        ecall                       # print space

        addi    s2, s2, 1
        j       print_loop

done:
        li      a0, 0
        li      a7, 10
        ecall
