# fibonacci.s — print the first 10 Fibonacci numbers
#
# Registers:
#   s0 = counter
#   s1 = F(n-1)
#   s2 = F(n)
#   t0 = scratch

        .text
        .globl main
main:
        li      s0, 0           # counter = 0
        li      s1, 0           # F(0) = 0
        li      s2, 1           # F(1) = 1
        li      s3, 10          # loop bound

loop:
        bge     s0, s3, done    # if counter >= 10, done

        li      a7, 1           # syscall: print_int
        mv      a0, s1
        ecall

        li      a7, 11          # syscall: print_char (newline)
        li      a0, 10
        ecall

        # F(n+1) = F(n-1) + F(n)
        add     t0, s1, s2
        mv      s1, s2
        mv      s2, t0

        addi    s0, s0, 1
        j       loop

done:
        li      a0, 0
        li      a7, 10
        ecall
