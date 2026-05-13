# fp_test.s — basic floating-point arithmetic smoke test
#
# Computes: result = (3.0 + 4.0) * 0.5  == 3.5
# Then prints result via syscall 2 (print_float from fa0).

        .data
msg1:   .string "FP result: "
msg2:   .string "\n"
a_val:  .float  3.0
b_val:  .float  4.0
half:   .float  0.5

        .text
        .globl main
main:
        # load constants
        la      t0, a_val
        flw     fa0, 0(t0)      # fa0 = 3.0

        la      t1, b_val
        flw     fa1, 0(t1)      # fa1 = 4.0

        la      t2, half
        flw     fa2, 0(t2)      # fa2 = 0.5

        # fa0 = (3.0 + 4.0) * 0.5 = 3.5
        fadd.s  fa0, fa0, fa1
        fmul.s  fa0, fa0, fa2

        # print "FP result: "
        la      a0, msg1
        li      a7, 4
        ecall

        # print_float: fa0 already holds 3.5
        li      a7, 2
        ecall

        # newline
        la      a0, msg2
        li      a7, 4
        ecall

        li      a0, 0
        li      a7, 10
        ecall
