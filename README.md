# OARS — Oxide Assembler and Runtime Simulator

> OARS is what RARS would be if it were rewritten today — a single double-clickable binary for Windows, macOS, and Linux. No Java, no installer.

OARS follows the MARS → RARS lineage, keeping the same educational mission and RARS-compatible memory layout so existing `.s` files run without modification.

## Download

Get the latest release from the **[Releases page](../../releases/latest)** and extract the archive — you get a single executable, nothing else to install.

| Platform | File |
| --- | --- |
| Windows (64-bit) | `oars-windows-x86_64.zip` |
| macOS — Apple Silicon (M1/M2/M3) | `oars-macos-arm.tar.gz` |
| macOS — Intel | `oars-macos-intel.tar.gz` |
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

Pseudo-instructions: `li`, `la`, `mv`, `not`, `neg`, `nop`, `j`, `jr`, `ret`, `call`, `beqz`, `bnez`, `blez`, `bgez`, `bltz`, `bgtz`, `bgt`, `ble`, `bgtu`, `bleu`, `seqz`, `snez`, `sltz`, `sgtz`.

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

## License

OARS is released under the [MIT License](LICENSE).  
Copyright © 2025 Nathan Hutchins.

OARS is inspired by [RARS](https://github.com/TheThirdOne/rars) (also MIT) and the original MARS simulator.
