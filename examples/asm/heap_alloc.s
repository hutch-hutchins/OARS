# heap_alloc.s — dynamic heap allocation via sbrk (syscall 9)
#
# Allocates an array of 8 integers on the heap using sbrk,
# fills it with values 1..8, then prints each element.

        .text
        .globl main

main:
        # sbrk(32): allocate 8 words (32 bytes) on the heap
        li      a0, 32
        li      a7, 9
        ecall                       # a0 = pointer to allocated block
        mv      s0, a0              # s0 = base address of array
        li      s1, 8               # s1 = array length (loop limit)

        # Fill array: mem[i] = i+1  for i in 0..7
        li      s2, 0               # s2 = index i

heap_fill:
        bge     s2, s1, heap_print
        addi    t0, s2, 1           # value = i + 1
        slli    t1, s2, 2           # byte offset = i * 4
        add     t1, s0, t1
        sw      t0, 0(t1)
        addi    s2, s2, 1
        j       heap_fill

heap_print:
        li      s2, 0               # reset index

heap_ploop:
        bge     s2, s1, heap_end
        slli    t1, s2, 2
        add     t1, s0, t1
        lw      a0, 0(t1)
        li      a7, 1
        ecall                       # print integer

        li      a0, ' '
        li      a7, 11
        ecall                       # print space character

        addi    s2, s2, 1
        j       heap_ploop

heap_end:
        li      a0, '\n'
        li      a7, 11
        ecall                       # print newline

        li      a0, 0
        li      a7, 10
        ecall
