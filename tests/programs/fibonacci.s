# fibonacci.s — print the first 10 Fibonacci numbers
#
# Registers:
#   s0 = counter (0..9)
#   s1 = F(n-1)
#   s2 = F(n)
#   t0 = scratch

        .text
        .globl main
main:
        li      s0, 0           # counter = 0
        li      s1, 0           # F(-1) = 0
        li      s2, 1           # F(0)  = 1

loop:
        bge     s0, 10, done    # if counter >= 10, done

        # print current fibonacci number
        li      a7, 1           # syscall: print_int
        mv      a0, s1
        ecall

        li      a7, 11          # syscall: print_char  (newline)
        li      a0, '\n'
        ecall

        # advance: F(n+1) = F(n-1) + F(n)
        add     t0, s1, s2
        mv      s1, s2
        mv      s2, t0

        addi    s0, s0, 1       # counter++
        j       loop

done:
        li      a7, 10          # syscall: exit
        ecall
