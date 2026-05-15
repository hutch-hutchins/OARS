# fibonacci64.s — First 10 Fibonacci numbers computed with 64-bit registers
#
# Demonstrates that the RV64I engine executes standard programs identically
# to RV32I for values that fit in 32 bits, while carrying full 64-bit state.
# F(0)=0  F(1)=1  ...  F(9)=34

        .text
main:
        li      t0, 0           # prev  = F(0)
        li      t1, 1           # curr  = F(1)
        li      t2, 10          # count = 10
        li      t3, 0           # i = 0
loop:
        bge     t3, t2, done

        mv      a0, t0
        li      a7, 1           # print_int(prev)
        ecall

        li      a0, '\n'
        li      a7, 11          # print_char('\n')
        ecall

        add     t4, t0, t1      # next = prev + curr  (64-bit ADD)
        mv      t0, t1
        mv      t1, t4
        addi    t3, t3, 1
        j       loop
done:
        li      a0, 0
        li      a7, 10          # exit(0)
        ecall
