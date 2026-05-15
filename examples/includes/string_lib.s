# string_lib.s — Reusable string subroutines
#
# Subroutines provided:
#   str_len   a0=ptr  →  a0 = strlen(ptr)   (clobbers t0)
#   str_upper a0=ptr  →  modifies string in place, uppercases a..z  (clobbers t0, t1)
#
# Usage:  .include "string_lib.s"

# ── str_len(a0) → a0 ──────────────────────────────────────────────────────────
# Counts bytes until a null terminator.

str_len:
        mv      t0, a0          # t0 = pointer
        li      a0, 0           # length = 0
sl_loop:
        lb      t1, 0(t0)
        beqz    t1, sl_done
        addi    a0, a0, 1
        addi    t0, t0, 1
        j       sl_loop
sl_done:
        ret

# ── str_upper(a0) — uppercase a..z in place ───────────────────────────────────
# Walks the string byte by byte; converts 'a'-'z' → 'A'-'Z' by clearing bit 5.

str_upper:
        mv      t0, a0          # t0 = pointer
su_loop:
        lb      t1, 0(t0)
        beqz    t1, su_done
        li      t2, 'a'
        blt     t1, t2, su_next
        li      t2, 'z'
        bgt     t1, t2, su_next
        andi    t1, t1, 0xDF    # clear bit 5: lowercase → uppercase
        sb      t1, 0(t0)
su_next:
        addi    t0, t0, 1
        j       su_loop
su_done:
        ret
