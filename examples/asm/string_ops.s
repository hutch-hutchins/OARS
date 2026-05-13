# string_ops.s — null-terminated string subroutines
#
# Implements strlen and str_reverse as subroutines, then demonstrates them
# on a fixed string.  Illustrates byte-level memory access (lb/sb) and
# pointer walking.
#
# Expected output:
#   Length: 5
#   olleh

        .text
        .globl main

# int strlen(char *s)  — a0 = pointer, returns length in a0
strlen:
        mv      t0, a0              # t0 = pointer
        li      t1, 0               # t1 = count
strlen_loop:
        lb      t2, 0(t0)
        beqz    t2, strlen_done
        addi    t1, t1, 1
        addi    t0, t0, 1
        j       strlen_loop
strlen_done:
        mv      a0, t1
        ret

# void str_reverse(char *s, int len)  — a0 = ptr, a1 = length
# reverses string in-place
str_reverse:
        addi    sp, sp, -8
        sw      ra, 4(sp)
        sw      s0, 0(sp)

        mv      t0, a0              # t0 = left pointer
        add     t1, a0, a1
        addi    t1, t1, -1          # t1 = right pointer (last char)

rev_loop:
        bge     t0, t1, rev_done
        lb      t2, 0(t0)           # t2 = left char
        lb      t3, 0(t1)           # t3 = right char
        sb      t3, 0(t0)           # *left = right char
        sb      t2, 0(t1)           # *right = left char
        addi    t0, t0, 1
        addi    t1, t1, -1
        j       rev_loop

rev_done:
        lw      ra, 4(sp)
        lw      s0, 0(sp)
        addi    sp, sp, 8
        ret

main:
        la      s0, mystr           # s0 = pointer to string

        # Print "Length: "
        la      a0, lbl_len
        li      a7, 4
        ecall

        # Compute and print strlen
        mv      a0, s0
        call    strlen
        mv      s1, a0              # s1 = length

        li      a7, 1
        ecall                       # print length

        li      a0, '\n'
        li      a7, 11
        ecall

        # Reverse the string
        mv      a0, s0
        mv      a1, s1
        call    str_reverse

        # Print reversed string
        mv      a0, s0
        li      a7, 4
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        li      a0, 0
        li      a7, 10
        ecall

        .data
mystr:  .asciz  "hello"
lbl_len:.asciz  "Length: "
