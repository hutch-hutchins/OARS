# OARS — Oxide Assembler and Runtime Simulator

> OARS is what RARS would be if it were rewritten today — a single double-clickable binary for Windows, macOS, and Linux. No Java, no installer.

OARS follows the MARS → RARS lineage, keeping the same educational mission and RARS-compatible memory layout so existing `.s` files run without modification.

## Download

Get the latest release from the **[Releases page](../../releases/latest)** and extract the archive — you get a single executable, nothing else to install.

| Platform | File |
|---|---|
| Windows (64-bit) | `oars-windows-x86_64.zip` |
| macOS — Apple Silicon (M1/M2/M3) | `oars-macos-arm.tar.gz` |
| macOS — Intel | `oars-macos-intel.tar.gz` |
| Linux (x86-64) | `oars-linux-x86_64.tar.gz` |

> **macOS:** On first launch, right-click → Open to bypass the Gatekeeper warning, or run `xattr -cr oars` in Terminal.

## Quick Start — GUI

1. Launch `oars.exe` (Windows) or `oars` (macOS / Linux).
2. Type or paste your RISC-V assembly in the editor pane.
3. Click **Assemble & Run** in the toolbar — output appears in the Console tab.
4. Use **Step** and **Backstep** to single-step through your program and undo steps.

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
|----|---------|-----------|--------|
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
|-----------|-------------|
| **RV32I** | All base integer instructions |
| **RV32M** | `mul`, `mulh`, `mulhsu`, `mulhu`, `div`, `divu`, `rem`, `remu` |
| **RV32F** | Single-precision FP — `flw`, `fsw`, arithmetic, conversions, comparisons |
| **RV32D** | Double-precision FP — `fld`, `fsd`, arithmetic, conversions, comparisons |
| **Zicsr** | `csrrw`, `csrrs`, `csrrc` + immediate variants |

Pseudo-instructions: `li`, `la`, `mv`, `not`, `neg`, `nop`, `j`, `jr`, `ret`, `call`, `beqz`, `bnez`, `blez`, `bgez`, `bltz`, `bgtz`, `bgt`, `ble`, `bgtu`, `bleu`, `seqz`, `snez`, `sltz`, `sgtz`.

## Memory Layout

| Region | Base Address |
|--------|-------------|
| Text (code) | `0x0040_0000` |
| Data | `0x1001_0000` |
| Stack top | `0x7FFF_EFFC` |

## Building from Source

Requires Rust 1.75 or later.

```sh
git clone https://github.com/hutch-hutchins/OARS.git
cd OARS
cargo build --release
```

The binary will be at `target/release/oars` (or `oars.exe` on Windows).
