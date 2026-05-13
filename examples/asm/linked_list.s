# linked_list.s — singly linked list using sbrk heap allocation
#
# Each node is 8 bytes:  word value (offset 0), word next_ptr (offset 4).
# Builds a list [10, 20, 30], then traverses from head to tail printing values.
# Demonstrates heap allocation, pointer arithmetic, and struct-like memory layout.

        .text
        .globl main

# alloc_node(value in a0) -> node ptr in a0
# allocates 8 bytes on heap, stores value, sets next = 0
alloc_node:
        addi    sp, sp, -8
        sw      ra, 4(sp)
        sw      a0, 0(sp)           # save value argument

        li      a0, 8
        li      a7, 9
        ecall                       # sbrk(8) -> a0 = node ptr

        mv      t0, a0              # t0 = new node ptr
        lw      t1, 0(sp)           # t1 = saved value
        sw      t1, 0(t0)           # node->value = value
        sw      zero, 4(t0)         # node->next  = NULL

        mv      a0, t0              # return node ptr
        lw      ra, 4(sp)
        addi    sp, sp, 8
        ret

main:
        # Allocate three nodes
        li      a0, 10
        call    alloc_node
        mv      s0, a0              # s0 = node_10

        li      a0, 20
        call    alloc_node
        mv      s1, a0              # s1 = node_20

        li      a0, 30
        call    alloc_node
        mv      s2, a0              # s2 = node_30

        # Link: node_10->next = node_20, node_20->next = node_30
        sw      s1, 4(s0)
        sw      s2, 4(s1)

        # Traverse the list and print each value
        mv      s3, s0              # s3 = current node (head)

ll_traverse:
        beqz    s3, ll_done
        lw      a0, 0(s3)           # a0 = node->value
        li      a7, 1
        ecall                       # print integer

        li      a0, '\n'
        li      a7, 11
        ecall                       # print newline

        lw      s3, 4(s3)           # s3 = node->next
        j       ll_traverse

ll_done:
        li      a0, 0
        li      a7, 10
        ecall
