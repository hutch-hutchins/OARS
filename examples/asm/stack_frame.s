# stack_frame.s — function calls and stack frame management
#
# Computes sum_of_squares(n) = 1^2 + 2^2 + ... + n^2 for n = 5
# using a helper function that demonstrates a full ABI-compliant stack frame:
#   - callee saves ra and s-registers
#   - caller passes arguments in a0

        .text
        .globl main

# int sum_of_squares(int n)
# a0 = n  →  returns result in a0
sum_of_squares:
        addi    sp, sp, -16         # allocate frame
        sw      ra,  12(sp)         # save return address
        sw      s0,   8(sp)         # save callee-saved regs
        sw      s1,   4(sp)

        mv      s0, a0              # s0 = n (loop counter)
        li      s1, 0               # s1 = accumulator

sos_loop:
        beqz    s0, sos_done
        mul     t0, s0, s0          # t0 = s0^2
        add     s1, s1, t0          # acc += s0^2
        addi    s0, s0, -1
        j       sos_loop

sos_done:
        mv      a0, s1              # return accumulator

        lw      ra,  12(sp)
        lw      s0,   8(sp)
        lw      s1,   4(sp)
        addi    sp, sp, 16
        ret

main:
        li      a0, 5               # n = 5
        call    sum_of_squares      # a0 = 1+4+9+16+25 = 55

        mv      a1, a0
        la      a0, msg
        li      a7, 4
        ecall                       # print "sum_of_squares(5) = "

        mv      a0, a1
        li      a7, 1
        ecall                       # print 55

        la      a0, newline
        li      a7, 4
        ecall

        li      a0, 0
        li      a7, 10
        ecall

        .data
msg:    .asciz  "sum_of_squares(5) = "
newline:.asciz  "\n"
