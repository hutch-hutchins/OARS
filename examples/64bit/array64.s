# array64.s — Sum an array of eight 64-bit doublewords using LD
#
# Stores [1, 2, 3, 4, 5, 6, 7, 8] as .dword entries (8 bytes each).
# Loads each element with LD, accumulates a 64-bit sum, and prints it.
# Expected output: 36

        .text
main:
        la      a0, arr         # a0 = &arr[0]
        li      t0, 8           # element count
        li      t1, 0           # sum (64-bit accumulator)
        li      t2, 0           # i = 0
loop:
        bge     t2, t0, done
        ld      t3, 0(a0)       # load 64-bit doubleword
        add     t1, t1, t3      # sum += elem  (64-bit ADD)
        addi    a0, a0, 8       # advance pointer by 8 bytes
        addi    t2, t2, 1
        j       loop
done:
        mv      a0, t1
        li      a7, 1           # print_int(sum)
        ecall

        li      a0, '\n'
        li      a7, 11
        ecall

        li      a0, 0
        li      a7, 10          # exit(0)
        ecall

        .data
arr:    .dword  1, 2, 3, 4, 5, 6, 7, 8
