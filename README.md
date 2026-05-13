# OARS — Oxide Assembler and Runtime Simulator

![OARS](assets/banner.png)

> OARS is what RARS would be if it were rewritten today — a single double-clickable binary for Windows, macOS, and Linux. No Java, no installer.

OARS follows the MARS → RARS lineage, keeping the same educational mission and RARS-compatible memory layout so existing `.s` files run without modification.

## Download

Get the latest release from the **[Releases page](../../releases/latest)** and extract the archive — you get a single executable, nothing else to install.

| Platform | File |
| --- | --- |
| Windows (64-bit) | `oars-windows-x86_64.zip` |
| macOS — Apple Silicon (M1/M2/M3) | `oars-macos-arm.tar.gz` |
| Linux (x86-64) | `oars-linux-x86_64.tar.gz` |

> **macOS:** On first launch, right-click → Open to bypass the Gatekeeper warning, or run `xattr -cr oars` in Terminal.

## Quick Start — GUI

1. Launch `oars.exe` (Windows) or `oars` (macOS / Linux).
2. Type or paste your RISC-V assembly in the **Editor** tab.
3. Click **Assemble** in the toolbar — the view switches to the **Text Segment** tab showing each instruction's address and machine code.
4. Click **Run** to execute, or use **Step** / **Backstep** to single-step through your program.
5. Watch register values update in real time in the right panel — changed registers are highlighted green.

## The Interface

```text
┌─ Menu bar (File / Help) ───────────────────────────────────────────────┐
├─ Toolbar: Assemble | Run | Step | Backstep | Pause | Reset | status ───┤
│                                                    │                   │
│  ┌─ Editor ──── Text Segment ─────────────────┐  │  Integer           │
│  │                                             │  │  Float             │
│  │   Centre panel — tabs switch between:       │  │  CSR               │
│  │   • Editor: write and edit your .s file     │  │                    │
│  │   • Text Segment: assembled instructions    │  │  (register panel,  │
│  │     with address, machine code, and source  │  │   right side)      │
│  │                                             │  │                    │
│  └─────────────────────────────────────────────┘  │                   │
│                                                    │                   │
│  ┌─ Console ──── Memory ───────────────────────────────────────────────┤
│  │                                                                     │
│  │   Bottom panel — tabs switch between:                               │
│  │   • Console: program output and stdin input                         │
│  │   • Memory: hex dump of memory with address, 4 words, ASCII        │
│  └─────────────────────────────────────────────────────────────────────┘
```

### Toolbar buttons

| Button | When enabled | Action |
| --- | --- | --- |
| **Assemble** | Always | Parse and assemble the source; switches to Text Segment on success |
| **Run** | After assembling | Run at full speed until halt or breakpoint |
| **Step** | Assembled, not running | Execute one instruction |
| **Backstep** | Steps in history | Undo the last instruction |
| **Pause** | While running | Pause execution |
| **Reset** | After assembling | Re-assemble and reset CPU state |

### Register panel (right side)

Three tabs show all architectural state:

- **Integer** — `x0`–`x31` with ABI names (`zero`, `ra`, `sp`, …), hex and decimal values
- **Float** — `f0`–`f31` with ABI names (`ft0`, `fa0`, `fs0`, …), hex and float values
- **CSR** — PC, `fflags`, `frm`, `fcsr`, `cycle`, `instret`

Registers that changed on the last step or run burst are highlighted **green**.

### Memory viewer (bottom panel → Memory tab)

Displays memory as a hex dump — 16 bytes (4 words) per row. Jump buttons at the top navigate instantly to the `.text`, `.data`, or `stack` regions. The row containing the current PC is highlighted yellow.

### Help

**Help → Instruction Reference** opens a scrollable reference for every supported instruction — pseudo-instructions, RV32I, RV32M, RV32F, RV32D, Zicsr, assembler directives, and syscalls — with a description and example for each.

## Quick Start — Command Line

```sh
oars program.s                        # assemble and run
oars program.s --dump-registers       # print integer registers on exit
oars program.s --dump-fp-registers    # print FP registers on exit
oars program.s --max-steps 100000     # cap execution at N instructions
oars program.s --start-at-main        # begin at label `main` instead of first instruction
oars program.s --telemetry            # emit instruction count + exit code as JSON
```

## Example Programs

Ready-to-run examples in [`examples/asm/`](examples/asm/) cover the advanced instruction extensions:

| File | Extensions used | What it does |
| --- | --- | --- |
| [`gcd.s`](examples/asm/gcd.s) | RV32M — `rem` | Euclidean GCD algorithm: `gcd(48, 18) = 6` |
| [`integer_power.s`](examples/asm/integer_power.s) | RV32M — `mul`, `mulh`, `mulhu` | Exponentiation by squaring; `mulhu` overflow detection |
| [`quadratic.s`](examples/asm/quadratic.s) | RV32F — `flw`, `fmul.s`, `fsub.s`, `fsqrt.s`, `fdiv.s`, `fneg.s`, `flt.s`, `fcvt.s.w` | Quadratic formula solver: roots of x²−5x+6=0 |
| [`dot_product.s`](examples/asm/dot_product.s) | RV32F — `fmadd.s`, `flw` | Dot product using fused multiply-add (FMA) |
| [`newton_sqrt.s`](examples/asm/newton_sqrt.s) | RV32D — `fld`, `fadd.d`, `fmul.d`, `fdiv.d`, `fabs.d`, `fsqrt.d` | Newton-Raphson sqrt(2) compared to hardware `fsqrt.d` |
| [`csr_benchmark.s`](examples/asm/csr_benchmark.s) | Zicsr — `csrr instret`; RV32M — `mul` | Instruction-count benchmarking: loop vs. Gauss formula |
| [`stack_frame.s`](examples/asm/stack_frame.s) | ABI — `call`/`ret`, callee-saved regs, stack frame | `sum_of_squares(5) = 55` via a fully ABI-compliant subroutine |
| [`factorial.s`](examples/asm/factorial.s) | ABI — recursive calls, `sw`/`lw` on stack | Recursive `10! = 3628800`; demonstrates saving `ra` and `a0` across calls |
| [`heap_alloc.s`](examples/asm/heap_alloc.s) | Syscall 9 — `sbrk` | Dynamic heap allocation: fill and print an 8-element array |

Open any file in OARS, click **Assemble**, then **Run** to see the output in the Console panel.

## Your First Program

```asm
# hello.s
.data
msg:  .asciiz "Hello, RISC-V!\n"

.text
main:
    li   a7, 4          # syscall 4 = print string
    la   a0, msg
    ecall
    li   a7, 10         # syscall 10 = exit
    ecall
```

## Syscall Reference

| a7 | Service | Arguments | Result |
| --- | --- | --- | --- |
| 1 | Print integer | `a0` = value | — |
| 2 | Print float | `fa0` = value | — |
| 3 | Print double | `fa0` = value | — |
| 4 | Print string | `a0` = address of null-terminated string | — |
| 5 | Read integer | — | `a0` |
| 6 | Read float | — | `fa0` |
| 7 | Read double | — | `fa0` |
| 8 | Read string | `a0` = buffer address, `a1` = max bytes | — |
| 10 | Exit | — | — |
| 11 | Print character | `a0` = ASCII code | — |
| 12 | Read character | — | `a0` |
| 34 | Print integer (hex) | `a0` = value | — |
| 35 | Print integer (binary) | `a0` = value | — |
| 36 | Print unsigned integer | `a0` = value | — |
| 93 | Exit with code | `a0` = exit code | — |

## ISA Coverage

| Extension | Instructions |
| --- | --- |
| **RV32I** | All base integer instructions |
| **RV32M** | `mul`, `mulh`, `mulhsu`, `mulhu`, `div`, `divu`, `rem`, `remu` |
| **RV32F** | Single-precision FP — `flw`, `fsw`, arithmetic, conversions, comparisons |
| **RV32D** | Double-precision FP — `fld`, `fsd`, arithmetic, conversions, comparisons |
| **Zicsr** | `csrrw`, `csrrs`, `csrrc` + immediate variants |

Pseudo-instructions: `li`, `la`, `mv`, `not`, `neg`, `nop`, `j`, `jr`, `ret`, `call`, `beqz`, `bnez`, `blez`, `bgez`, `bltz`, `bgtz`, `bgt`, `ble`, `bgtu`, `bleu`, `seqz`, `snez`, `sltz`, `sgtz`, `fmv.s`, `fmv.d`, `fabs.s`, `fabs.d`, `fneg.s`, `fneg.d`, `csrr`, `csrw`, `csrs`, `csrc`, `csrwi`, `csrsi`, `csrci`.

## Memory Layout

| Region | Base Address |
| --- | --- |
| Text (code) | `0x0040_0000` |
| Data | `0x1001_0000` |
| Heap | `0x1004_0000` |
| Stack top | `0x7FFF_EFFC` |

## Building from Source

Requires Rust 1.75 or later.

```sh
git clone https://github.com/hutch-hutchins/OARS.git
cd OARS
cargo build --release
```

The binary will be at `target/release/oars` (or `oars.exe` on Windows).

## Roadmap

### v0.4.0 — Current Release ✓

#### Export

- [x] `File → Export Flat Binary…` — saves raw text-segment bytes (load at `TEXT_BASE`)
- [x] `File → Export ELF…` — minimal ELF32 RISC-V executable with PT_LOAD segments (text r-x, data rw-)

#### Examples & Tests

- [x] 3 new ABI examples: `stack_frame.s` (subroutine with saved regs), `factorial.s` (recursive), `heap_alloc.s` (sbrk)
- [x] `.asciz` directive support (alias for `.asciiz` / `.string`)
- [x] `ret` / `jr` pseudo-op fix: now correctly emits `jalr rd, 0(rs1)`
- [x] Integration test suite grown to 13 tests, all passing

### v0.3.0 — Prior Release ✓

#### ISA & Core

- [x] RV32I base integer ISA
- [x] RV32M multiply / divide (`mul`, `mulh`, `mulhu`, `mulhsu`, `div`, `divu`, `rem`, `remu`)
- [x] RV32F / RV32D single- and double-precision floating point
- [x] Zicsr control-and-status register instructions (`csrrw`, `csrrs`, `csrrc` + immediate variants)
- [x] Pseudo-instruction expansion (`li`, `la`, `mv`, `call`, `ret`, branch pseudos, CSR pseudos, FP pseudos, …)
- [x] Assembler directives (`.text`, `.data`, `.word`, `.half`, `.byte`, `.float`, `.double`, `.asciiz`, `.string`, `.space`, `.align`, …)
- [x] Syscalls 1–12, 34, 35, 36, 93 (print/read integer, float, double, string, char; hex, binary, unsigned; exit with code)
- [x] CLI runner (`--dump-registers`, `--dump-fp-registers`, `--max-steps`, `--start-at-main`, `--telemetry`)

#### GUI & Debugger

- [x] egui / eframe native GUI — single double-clickable binary, no installer
- [x] Editor tab with line numbers and RISC-V syntax highlighting
- [x] Inline error markers — squiggles on the offending token, red background on the line
- [x] Find / replace in editor (Ctrl+F, case-sensitive toggle)
- [x] Multiple file tabs — open more than one `.s` file at a time
- [x] Text segment tab — address, machine code, source, labels, breakpoint toggles
- [x] Register panel — Integer / Float / CSR tabs, changed-register highlighting (green)
- [x] Memory viewer — hex dump, jump-to-region buttons, PC row highlighted yellow
- [x] Data segment viewer — label column, 4-word rows, ASCII column
- [x] Stack viewer — `sp` indicator, used-vs-free highlighting
- [x] Watch panel — pin registers or memory addresses; values update on every step
- [x] Backstep — undo the last instruction including memory writes (256-entry ring buffer)
- [x] Breakpoints — click gutter dot or text-segment row to toggle
- [x] Run-speed slider (logarithmic, 1 Hz → 2 MHz)
- [x] Light / dark theme toggle
- [x] Virtual terminal console — `read_char` captures a single keypress inline with a block cursor; line-buffered reads show an inline input field
- [x] Help → Instruction Reference (tabbed: Pseudo / RV32I / RV32M / RV32F / RV32D / Zicsr / Directives / Syscalls)
- [x] Help → About OARS
- [x] File open / save dialog (`.s` / `.asm` filter)
- [x] GitHub Actions release builds (Windows x86-64, macOS ARM, Linux x86-64)

#### v0.3.0 Examples & Tests

- [x] 11 ready-to-run example programs in `examples/asm/` covering RV32M, RV32F, RV32D, Zicsr, and interactive console I/O
- [x] Integration test suite — 10 tests, all passing

### Future

- [ ] Export assembled binary (ELF or raw flat binary)
- [ ] RV64I 64-bit mode

## License

OARS is released under the [MIT License](LICENSE).  
Copyright © 2025 Nathan Hutchins.

OARS is inspired by [RARS](https://github.com/TheThirdOne/rars) (also MIT) and the original MARS simulator.
