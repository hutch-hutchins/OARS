# console_io.s — interactive I/O using ECALL
#
# Demonstrates every console syscall in one program:
#   syscall  4  print_string   — print a null-terminated string from memory
#   syscall  1  print_int      — print an integer
#   syscall 11  print_char     — print a single character (newline here)
#   syscall  5  read_int       — read an integer typed by the user
#   syscall  8  read_string    — read a line of text into a buffer
#   syscall 12  read_char      — read a single keypress
#   syscall 10  exit
#
# Sample session:
#   Enter your name: Alice
#   Enter a number : 7
#   Press any key  : (user presses 'y')
#   Hello, Alice!
#   Your number doubled is 14
#   You pressed: y

        .data
prompt_name:  .string "Enter your name: "
prompt_num:   .string "Enter a number : "
prompt_key:   .string "Press any key  : "

msg_hello:    .string "Hello, "
msg_exclaim:  .string "!\n"
msg_doubled:  .string "Your number doubled is "
msg_pressed:  .string "You pressed: "
newline:      .string "\n"

name_buf:     .space  32          # 32-byte buffer for the name

        .text
        .globl main
main:
        # ── ask for name (read_string) ────────────────────────────────────────
        la      a0, prompt_name
        li      a7, 4
        ecall                       # print "Enter your name: "

        la      a0, name_buf        # a0 = buffer address
        li      a1, 32              # a1 = max bytes (including null terminator)
        li      a7, 8
        ecall                       # read_string → fills name_buf

        # strip trailing newline written by read_string
        la      t0, name_buf
strip_loop:
        lb      t1, 0(t0)
        beqz    t1, strip_done      # null terminator — stop
        li      t2, 10
        beq     t1, t2, strip_nl    # newline — replace with null
        addi    t0, t0, 1
        j       strip_loop
strip_nl:
        sb      zero, 0(t0)         # overwrite '\n' with '\0'
strip_done:

        # ── ask for a number (read_int) ───────────────────────────────────────
        la      a0, prompt_num
        li      a7, 4
        ecall                       # print "Enter a number : "

        li      a7, 5
        ecall                       # read_int → a0
        mv      s0, a0              # s0 = the number

        # ── ask for a single keypress (read_char) ─────────────────────────────
        la      a0, prompt_key
        li      a7, 4
        ecall                       # print "Press any key  : "

        li      a7, 12
        ecall                       # read_char → a0
        mv      s1, a0              # s1 = ASCII code of key pressed

        # ── print "Hello, <name>!\n" ──────────────────────────────────────────
        la      a0, msg_hello
        li      a7, 4
        ecall                       # print "Hello, "

        la      a0, name_buf
        li      a7, 4
        ecall                       # print the name

        la      a0, msg_exclaim
        li      a7, 4
        ecall                       # print "!\n"

        # ── print "Your number doubled is <n*2>\n" ────────────────────────────
        la      a0, msg_doubled
        li      a7, 4
        ecall                       # print "Your number doubled is "

        slli    a0, s0, 1           # a0 = s0 * 2  (shift left by 1)
        li      a7, 1
        ecall                       # print_int

        la      a0, newline
        li      a7, 4
        ecall

        # ── print "You pressed: <char>\n" ─────────────────────────────────────
        la      a0, msg_pressed
        li      a7, 4
        ecall                       # print "You pressed: "

        mv      a0, s1
        li      a7, 11
        ecall                       # print_char (the key the user pressed)

        la      a0, newline
        li      a7, 4
        ecall

        # ── exit ──────────────────────────────────────────────────────────────
        li      a0, 0
        li      a7, 10
        ecall
