# RISC-V Native Simulator — Getting Started & Project Plan

> A native, single-binary RISC-V assembler and simulator for students on Windows, macOS, and Linux.
> No JVM. No install wizard. Download and run.

---

## Table of Contents

1. [Vision & Motivation](#1-vision--motivation)
2. [Prior Art — What Already Exists](#2-prior-art--what-already-exists)
3. [What We Are Building](#3-what-we-are-building)
4. [Feature Gap Analysis](#4-feature-gap-analysis)
5. [Language & Technology Choice](#5-language--technology-choice)
6. [Architecture](#6-architecture)
7. [Implementation Phases](#7-implementation-phases)
8. [Getting the Reference Projects](#8-getting-the-reference-projects)
9. [Starting Development](#9-starting-development)
10. [Distribution Target](#10-distribution-target)

---

## 1. Vision & Motivation

RARS (RISC-V Assembler and Runtime Simulator) is the best educational RISC-V simulator available — it is a direct fork of MARS (MIPS Assembler and Runtime Simulator), has 35+ syscalls, full floating-point register support, a mature Swing GUI, and is well-understood by students who learned MIPS. It works.

**The problem:** it requires a JVM. Students on a fresh Windows laptop must:

1. Install Java 8+
2. Download `rars.jar`
3. Run `java -jar rars.jar` from a terminal (or figure out file associations)

This creates a 15-minute barrier on day one of an assembly course. A native binary eliminates it entirely.

**Secondary goal:** while rebuilding, fill in the gaps RARS has compared to Ripes (pipeline visualization options, cache simulation, CLI telemetry for auto-grading) without losing what makes RARS better than Ripes (full FP support, 35+ syscalls, MARS familiarity).

---

## 2. Prior Art — What Already Exists

### 2.1 RARS — RISC-V Assembler and Runtime Simulator

**The primary reference. Model this project on RARS.**

| | |
|---|---|
| Repository | https://github.com/TheThirdOne/rars |
| Language | Java 8+ |
| GUI | Java Swing |
| License | MIT |
| Based on | MARS 4.5 (MIPS simulator, Pete Sanderson & Kenneth Vollmar) |

**What RARS does well:**
- Full two-pass assembler with macro support
- 35+ syscall handlers (print, read, file I/O, malloc, exit)
- Full RV32F and RV32D floating-point register file (f0–f31)
- CSR registers (CSRRW, CSRRS, etc.)
- Pseudo-instruction expansion (mv, li, la, not, neg, …)
- Interactive Swing IDE: editor, register window, memory window, console
- Backstepper (reverse execution, step backward)
- Multiple memory dump formats (binary, Intel HEX, hex text)
- CLI mode for headless/scripted use
- RV32 and RV64 support

**What RARS is missing:**
- Native binary (always needs JVM)
- Pipeline visualization
- Cache simulator
- CLI telemetry (cycle count, instruction count)
- Multiple processor variants (single-stage vs pipelined vs superscalar)
- Browser/WASM build

---

### 2.2 Ripes

**Reference for pipeline visualization, cache simulation, and processor variants.**

| | |
|---|---|
| Repository | https://github.com/mortbopet/Ripes |
| Language | C++20 |
| GUI | Qt 6.5+ |
| Build | CMake |
| Key submodule | https://github.com/mortbopet/VSRTL (hardware simulation library) |
| License | MIT |

**What Ripes does well:**
- 8 pipeline processor variants (single-stage → dual-issue superscalar)
- Hardware-accurate simulation via VSRTL (gate-level, not behavioral)
- Live circuit diagram with signal values
- Pipeline stage table (per-cycle view of what is in each stage)
- L1 cache simulator with hit/miss timeline chart
- CLI mode with cycle/instruction count telemetry
- Compressed instruction support (RV32C)
- ELF binary loading (load compiled C programs)
- WASM browser build via Emscripten

**What Ripes is missing vs RARS:**
- Floating-point: only stubs, not implemented in the processors
- Syscalls: ~15 vs RARS's 35+
- MARS/RARS lineage familiarity for students

**Ripes pipeline variants — worth replicating as teaching tools:**

| Variant | Teaching purpose |
|---|---|
| Single-stage | Baseline, no pipeline |
| RV5S (full 5-stage) | Forwarding + hazard detection — the standard model |
| RV5S_NO_FW | No forwarding — students see stall penalties |
| RV5S_NO_HZ | No hazard detection — students see incorrect results |
| RV5S_NO_FW_HZ | Neither — maximum stalls, maximum learning |
| RV5MC | 2-wide superscalar prototype |
| RV6S_DUAL | Dual-issue out-of-order fetch |

---

### 2.3 Venus

**Reference for web/browser deployment model.**

| | |
|---|---|
| Repository | https://github.com/kvakil/venus |
| Maintained fork | https://github.com/ThaumicMekanism/venus |
| Language | Java (Kotlin) |
| Runs as | Web app + VS Code extension |
| Used by | UC Berkeley CS 61C |

Zero install for students — runs in a browser tab. Lighter feature set than RARS. Good model for the WASM build target.

---

### 2.4 Jupiter

**Closest direct RARS alternative (also Java, also GUI+CLI).**

| | |
|---|---|
| Repository | https://github.com/andrescv/Jupiter |
| Language | Java |
| ISA | RV32IMF |

Similar scope to RARS, smaller community. Not worth forking, but good for comparison testing.

---

### 2.5 Spike (Official RISC-V ISA Reference Simulator)

**Use as a ground-truth oracle for correctness testing.**

| | |
|---|---|
| Repository | https://github.com/riscv-software-src/riscv-isa-sim |
| Language | C++ |
| Audience | Developers, researchers, hardware designers |
| Use in this project | Run the same programs through Spike and our simulator; compare register/memory state |

Not a classroom tool, but the authoritative reference implementation. If our output disagrees with Spike's, we are wrong.

---

### 2.6 WebRISC-V

**Reference for pipeline visualization UI design.**

| | |
|---|---|
| Repository | https://github.com/Mariotti94/WebRISC-V |
| URL | https://webriscv.dii.unisi.it |
| Language | PHP + web |

Five-stage pipelined datapath visualization. Good UI reference for how to display pipeline stage diagrams.

---

### 2.7 Other Rust RISC-V Implementations

These are CLI-only or research tools — none are classroom-ready, but useful for studying Rust patterns:

| Project | Repository | Notes |
|---|---|---|
| rvemu | https://github.com/d0iasm/rvemu | Rust, runs xv6/Linux, WASM build, CLI |
| rrs | https://gregchadwick.co.uk/blog/building-rrs-pt1/ | Rust, RV32IM, CLI, well-documented blog series |
| Terminus | https://github.com/shady831213/terminus | Rust, boots Linux |

---

## 3. What We Are Building

A Rust application that:

- Assembles `.s` RISC-V assembly files (RV32I/RV64I/M/F/D/C)
- Simulates execution with a visual GUI
- Ships as a **single self-contained binary** per platform
- Covers all RARS features students depend on (35+ syscalls, full FP, pseudo-instructions, backstepper)
- Adds Ripes features RARS lacks (pipeline variant selector, cache sim tab, CLI telemetry)
- Targets: `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`
- Optional stretch goal: WASM build for zero-install browser use

**Working name:** `riscv-sim` (rename before release)

---

## 4. Feature Gap Analysis

Features marked **RARS** are ported from RARS. Features marked **Ripes** are new additions inspired by Ripes. Features marked **New** fill gaps neither tool addresses well.

| Feature | RARS | Ripes | Priority | Source |
|---|---|---|---|---|
| RV32I base integer ISA | ✅ | ✅ | P0 | RARS |
| RV64I (64-bit variants) | ✅ | ✅ | P0 | RARS |
| RV32M/RV64M multiply/divide | ✅ | ✅ | P0 | RARS |
| RV32F/RV64F single-precision FP | ✅ | stubs only | P1 | RARS |
| RV32D/RV64D double-precision FP | ✅ | stubs only | P1 | RARS |
| RV32C compressed instructions | partial | ✅ | P2 | Ripes |
| CSR instructions (CSRRW, etc.) | ✅ | partial | P1 | RARS |
| Pseudo-instruction expansion | ✅ | ✅ | P0 | RARS |
| Macro support (.macro/.end_macro) | ✅ | ✗ | P1 | RARS |
| Two-pass assembler | ✅ | ✅ | P0 | both |
| GNU directives (.text, .data, .word…) | ✅ | ✅ | P0 | both |
| 35+ syscall handlers | ✅ | ~15 | P0 | RARS |
| Interactive GUI editor | ✅ | ✅ | P1 | RARS |
| Register window (int + FP + CSR) | ✅ | ✅ | P1 | RARS |
| Memory/data segment viewer | ✅ | ✅ | P1 | RARS |
| Console I/O panel | ✅ | ✅ | P1 | RARS |
| Backstepper (reverse execution) | ✅ | ✅ | P2 | RARS |
| Breakpoints | ✅ | ✅ | P2 | RARS |
| Memory dump formats | ✅ | partial | P2 | RARS |
| Pipeline stage selector | ✗ | ✅ | P2 | Ripes |
| Pipeline stage table view | ✗ | ✅ | P2 | Ripes |
| Cache simulator tab | ✗ | ✅ | P3 | Ripes |
| CLI mode | ✅ | ✅ | P1 | both |
| CLI telemetry (cycles, instr count) | ✗ | ✅ | P1 | Ripes |
| Native binary (no JVM) | ✗ | ✅ | P0 | — |
| WASM browser build | ✗ | ✅ | P3 | Ripes |
| ELF binary loading | ✗ | ✅ | P3 | Ripes |
| Auto-grading CLI output format | ✗ | partial | P2 | New |

---

## 5. Language & Technology Choice

### Rust (recommended)

| Concern | Choice | Rationale |
|---|---|---|
| Language | **Rust** (2021 edition) | Single binary, no runtime, memory safe, cross-compiles easily |
| GUI | **egui + eframe** | Immediate-mode, compiles to native and WASM from same code, ~5 MB overhead |
| Code editor widget | **egui_code_editor** | Syntax highlighting in egui |
| CLI parsing | **clap** (derive API) | Standard, ergonomic |
| FP arithmetic | **softfloat-wrapper** or native `f32`/`f64` | Native IEEE 754 sufficient for RV32F/D; softfloat for strict bit-exact behavior |
| Error handling | **thiserror** + **anyhow** | thiserror for library errors, anyhow for application errors |
| Serialization | **serde** + **serde_json** | Settings persistence, dump formats |
| Cross-compile CI | **GitHub Actions** + `cross` | Build all four targets from one Linux runner |

**Binary size target:** < 10 MB per platform (stripped release build with LTO).

### Go (alternative if contributor onboarding matters more)

Swap egui → **Fyne v2**, cargo → `go build -ldflags="-s -w"`. All module structure below is identical. Go binaries are ~12–18 MB but contributors ramp up in days rather than weeks.

---

## 6. Architecture

### Directory Layout

```
riscv-sim/
├── Cargo.toml
├── src/
│   ├── main.rs                  # Entry: parse args, launch GUI or CLI
│   ├── cli.rs                   # clap definitions + headless runner
│   │
│   ├── assembler/
│   │   ├── mod.rs
│   │   ├── lexer.rs             # Tokenizer
│   │   ├── parser.rs            # Statement / directive parser
│   │   ├── directives.rs        # .text .data .word .string .macro etc.
│   │   ├── macros.rs            # Macro pool + expansion
│   │   ├── symbol_table.rs      # Two-pass label resolution
│   │   └── codegen.rs           # Binary encoding (R/I/S/B/U/J formats)
│   │
│   ├── isa/
│   │   ├── mod.rs
│   │   ├── formats.rs           # Bit-field structs for each format
│   │   ├── pseudo.rs            # Pseudo-instruction expansion table
│   │   ├── rv32i.rs             # Base integer (37 instructions)
│   │   ├── rv64i.rs             # 64-bit variants
│   │   ├── rv32m.rs             # Multiply / divide
│   │   ├── rv32f.rs             # Single-precision FP
│   │   ├── rv32d.rs             # Double-precision FP
│   │   ├── rv32c.rs             # Compressed (C extension)
│   │   └── csr.rs               # CSR instructions
│   │
│   ├── hardware/
│   │   ├── mod.rs
│   │   ├── memory.rs            # Byte-addressed sparse memory map
│   │   ├── registers.rs         # x0–x31 integer registers
│   │   ├── fp_registers.rs      # f0–f31 floating-point registers
│   │   ├── csr.rs               # CSR register file
│   │   └── interrupts.rs        # Exception / trap handling
│   │
│   ├── simulator/
│   │   ├── mod.rs
│   │   ├── engine.rs            # Fetch-decode-execute loop
│   │   ├── pipeline.rs          # Optional: pipeline timing model
│   │   ├── backstrack.rs        # Undo / reverse-step history
│   │   ├── breakpoints.rs
│   │   └── syscalls/
│   │       ├── mod.rs
│   │       ├── io.rs            # print_int/str/char, read_int/str/char
│   │       ├── files.rs         # open, close, read, write, lseek
│   │       └── memory.rs        # malloc, free, sbrk, get_heap_end
│   │
│   ├── gui/
│   │   ├── mod.rs
│   │   ├── app.rs               # eframe App — top-level layout
│   │   ├── editor.rs            # Source editor pane
│   │   ├── toolbar.rs           # Run / Step / Step-Back / Reset / Speed
│   │   ├── text_segment.rs      # Disassembly + PC arrow
│   │   ├── registers.rs         # Integer + FP + CSR register panels
│   │   ├── memory_view.rs       # Data segment hex viewer
│   │   ├── console.rs           # stdin/stdout panel
│   │   ├── pipeline_view.rs     # Pipeline stage table (Ripes-inspired)
│   │   └── cache_view.rs        # Cache hit/miss display (Ripes-inspired)
│   │
│   └── util/
│       ├── binary.rs            # Hex / decimal / binary formatting
│       ├── error.rs             # Error types (source line, column)
│       └── dump.rs              # Memory dump: binary, hex, Intel HEX
│
├── tests/
│   ├── assembler.rs             # Assembler unit tests
│   ├── simulator.rs             # Execution correctness tests
│   ├── syscalls.rs
│   └── programs/                # .s files from RARS examples + new ones
│
├── examples/                    # Bundled example programs for students
│   ├── hello.s
│   ├── fibonacci.s
│   ├── bubble_sort.s
│   └── ...
│
└── dist/                        # CI artifact output (binaries)
```

### Key Design Decisions

**One enum per ISA group, not one file per opcode.**
RARS uses 149 Java classes loaded by reflection. In Rust, one `match` arm per instruction inside `rv32i.rs` is clearer, faster to compile, and easier to read.

```rust
// src/isa/rv32i.rs — all base integer instructions in one match
pub fn execute(op: Rv32iOp, state: &mut CpuState) -> Result<(), SimError> {
    match op {
        Rv32iOp::Add  => state.set_rd(state.rs1().wrapping_add(state.rs2())),
        Rv32iOp::Sub  => state.set_rd(state.rs1().wrapping_sub(state.rs2())),
        Rv32iOp::Addi => state.set_rd(state.rs1().wrapping_add(state.imm())),
        // ...
    }
}
```

**Sparse memory map.**
`BTreeMap<u32, u8>` — simple, correct, no large pre-allocation. Optimize later if profiling shows it matters (it won't at educational program scales).

**Immediate-mode GUI.**
egui redraws every frame by reading current CPU state. No Observer/Observable wiring needed. The GUI calls `engine.step()` on each clock tick. State flows one direction.

**Pipeline model is optional and additive.**
The behavioral engine (Phase 1) runs correctly without any pipeline model. The pipeline view (Phase 3) adds a cycle-accurate timing overlay on top — it does not change correctness.

---

## 7. Implementation Phases

### Phase 0 — Project Setup (Week 1)
- [ ] `cargo new riscv-sim --bin`
- [ ] Configure CI: GitHub Actions matrix for Windows / macOS / Linux
- [ ] Copy RARS `examples/*.s` as integration test inputs
- [ ] Golden-file test harness: run program, compare stdout + register state to expected output
- [ ] Write `lexer.rs` skeleton with unit tests

### Phase 1 — Headless CLI Assembler + Simulator (Weeks 2–6)
**Deliverable:** `riscv-sim program.s` runs from terminal, correct output.

- [ ] Lexer — tokenize `.s` files
- [ ] Parser — statements, labels, directives
- [ ] Symbol table — two-pass label resolution
- [ ] Macro expansion (`.macro` / `.end_macro`)
- [ ] Binary encoder — R/I/S/B/U/J formats
- [ ] Memory model + register file
- [ ] Fetch-decode-execute loop
- [ ] RV32I base instructions (37 opcodes)
- [ ] Pseudo-instructions (mv, li, la, j, beqz, bnez, nop, …)
- [ ] Syscall handlers — I/O group (print_int, print_str, print_char, read_int, read_str, read_char, exit, exit2)
- [ ] Syscall handlers — file I/O group (open, close, read, write)
- [ ] Syscall handlers — memory group (malloc, free, sbrk)
- [ ] Error reporting with source file, line number, column
- [ ] CLI flags: `--dump-registers`, `--max-steps N`, `--start-at-main`
- [ ] Integration tests against all RARS example programs

**Milestone check:** output matches RARS output for all example programs.

### Phase 2 — Full ISA Coverage (Weeks 7–9)
**Deliverable:** Passes extended test suite including FP and multiply.

- [ ] RV32M / RV64M — multiply and divide (8 opcodes)
- [ ] RV64I — 64-bit word variants (ADDW, SUBW, LWU, LD, SD, etc.)
- [ ] RV32F — single-precision FP (30+ opcodes)
- [ ] RV32D — double-precision FP (30+ opcodes)
- [ ] CSR instructions (CSRRW, CSRRS, CSRRC, CSRRWI, CSRRSI, CSRRCI)
- [ ] Interrupt / exception model (mcause, mepc, mtvec, mstatus)
- [ ] Backstepper — snapshot state per step, reverse-step restores previous snapshot
- [ ] Breakpoints — break on address, break on register write
- [ ] Memory dump formats (binary, hex text, Intel HEX, ASCII text)
- [ ] CLI telemetry output: cycle count, instruction count, final register state (JSON-friendly for auto-graders)

**Milestone check:** FP programs produce bit-exact output vs RARS.

### Phase 3 — GUI (Weeks 10–15)
**Deliverable:** Full desktop IDE, students never need the terminal.

- [ ] eframe application scaffold (window, menu bar, tab layout)
- [ ] Source editor with RISC-V syntax highlighting
- [ ] Assemble / Run / Step / Step-Back / Reset toolbar
- [ ] Run-speed slider (instructions per second)
- [ ] Console panel — live stdout, stdin prompt during simulation
- [ ] Text segment panel — disassembly list with PC arrow, breakpoint gutter
- [ ] Integer register panel — hex/decimal/binary toggle, green-flash on write
- [ ] FP register panel
- [ ] CSR register panel
- [ ] Data segment / memory viewer — address jump, hex display
- [ ] Labels / symbol table panel
- [ ] Error / warning message panel with source-line jump
- [ ] Pipeline stage selector — choose: single-stage, 5-stage full, 5-stage no-forward, 5-stage no-hazard
- [ ] Pipeline stage table — per-cycle view of instruction in each stage, stall/flush indicators
- [ ] Settings dialog — memory configuration, display radix, theme
- [ ] Help panel — instruction reference card

**Milestone check:** All RARS GUI workflows work. Pipeline selector changes cycle count visibly for a load-use hazard test program.

### Phase 4 — Polish + Distribution (Week 16)
- [ ] GitHub Actions release workflow — produce binaries on git tag push
- [ ] Windows: embed icon in `.exe` (`winres` crate)
- [ ] macOS: bundle `.app` with icon, code-sign if possible
- [ ] Linux: AppImage or plain binary + desktop file
- [ ] Student README — one page, download link, screenshot, first program
- [ ] Cache simulator tab (stretch — Ripes-inspired)
- [ ] WASM build target via `trunk` + `eframe` (stretch)

---

## 8. Getting the Reference Projects

Clone all three reference repositories before starting development. Read them; do not fork them.

```bash
# RARS — primary reference (Java, ~55,000 LOC)
git clone https://github.com/TheThirdOne/rars.git reference/rars

# Ripes — pipeline visualization + cache sim reference (C++/Qt, ~27,000 LOC)
git clone --recurse-submodules https://github.com/mortbopet/Ripes.git reference/ripes

# VSRTL — Ripes's hardware simulation library (C++, ~5,000 LOC)
# (already cloned as a submodule inside reference/ripes/external/VSRTL)

# Venus — web/browser deployment reference (Kotlin)
git clone https://github.com/ThaumicMekanism/venus.git reference/venus

# Jupiter — feature comparison reference (Java)
git clone https://github.com/andrescv/Jupiter.git reference/jupiter

# Spike — ground-truth correctness oracle (C++)
git clone https://github.com/riscv-software-src/riscv-isa-sim.git reference/spike
```

> Tip: put all reference clones in a `reference/` folder that is `.gitignore`d from the main project.

---

## 9. Starting Development

### Prerequisites

```bash
# Rust toolchain (stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable

# Add cross-compilation targets
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-apple-darwin        # macOS Intel
rustup target add aarch64-apple-darwin       # macOS Apple Silicon
rustup target add x86_64-unknown-linux-gnu

# cross — cross-compilation via Docker (optional, for Linux users building Windows)
cargo install cross

# trunk — for the WASM build target (Phase 4 stretch)
cargo install trunk
```

### Scaffold the Project

```bash
cargo new riscv-sim --bin
cd riscv-sim

# Create the module directory structure
mkdir -p src/assembler src/isa src/hardware src/simulator/syscalls src/gui src/util
mkdir -p tests/programs examples dist reference
```

### Initial Cargo.toml

```toml
[package]
name    = "riscv-sim"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "riscv-sim"
path = "src/main.rs"

[dependencies]
eframe  = "0.27"
egui    = "0.27"
egui_extras = "0.27"
egui_code_editor = "0.4"
clap    = { version = "4", features = ["derive"] }
thiserror = "1"
anyhow  = "1"
serde   = { version = "1", features = ["derive"] }
serde_json = "1"

[profile.release]
opt-level = 3
lto       = true
strip     = true
```

### First Commit Checklist

```
riscv-sim/
├── Cargo.toml            — dependencies above
├── src/
│   ├── main.rs           — "Hello RISC-V" placeholder, CLI arg skeleton
│   ├── assembler/mod.rs  — empty module stubs
│   ├── isa/mod.rs
│   ├── hardware/mod.rs
│   ├── simulator/mod.rs
│   ├── gui/mod.rs
│   └── util/mod.rs
├── tests/
│   └── programs/hello.s  — copied from RARS examples
├── .github/
│   └── workflows/
│       └── ci.yml        — cargo test on push
└── .gitignore            — target/ reference/
```

### Recommended Order of Attack

**Do not touch the GUI until Phase 2 is complete.** The GUI should be a thin display skin over a correct simulator, not a scaffolding for a broken one. Write a test for every syscall. Run every RARS example program through your simulator and diff the output before starting Phase 3.

```bash
# Run tests
cargo test

# Build release binary for current platform
cargo build --release

# Cross-compile for Windows from Linux/macOS
cross build --release --target x86_64-pc-windows-gnu

# Run a specific .s file (after Phase 1 is working)
./target/release/riscv-sim examples/hello.s

# Run with CLI telemetry (after Phase 2)
./target/release/riscv-sim --telemetry examples/fibonacci.s
```

### Correctness Testing Strategy

1. Copy all `.s` files from `reference/rars/test/` and `reference/rars/examples/`
2. Run each through RARS headless to capture golden output: `java -jar rars.jar nc me sm program.s`
3. Run each through your simulator and diff
4. Any divergence is a bug in your simulator
5. For FP programs, also compare against Spike

```bash
# Run RARS headless (requires JVM for this step only)
java -jar reference/rars/rars.jar nc me sm tests/programs/fibonacci.s > tests/golden/fibonacci.txt

# Run our simulator
./target/release/riscv-sim tests/programs/fibonacci.s > tests/out/fibonacci.txt

# Diff
diff tests/golden/fibonacci.txt tests/out/fibonacci.txt
```

---

## 10. Distribution Target

Students get a file from a GitHub Releases page. They download it, run it. Nothing else.

| Platform | Artifact | Size target |
|---|---|---|
| Windows 10/11 | `riscv-sim.exe` | < 10 MB |
| macOS (Intel) | `riscv-sim-macos-x64` | < 10 MB |
| macOS (Apple Silicon) | `riscv-sim-macos-arm64` | < 10 MB |
| Linux x86_64 | `riscv-sim-linux` | < 10 MB |
| Browser (stretch) | `riscv-sim.wasm` via web host | — |

GitHub Actions release workflow triggers on `git tag v*`, cross-compiles all four targets, uploads them as release assets.

Compare to current RARS student experience:
- **RARS today:** install JDK → download .jar → `java -jar rars.jar` (3 steps, 15 min barrier)
- **This project:** download `.exe` → double-click (1 step, 30 second barrier)

---

## References

| Project | URL |
|---|---|
| RARS source | https://github.com/TheThirdOne/rars |
| RARS original (MARS) | http://courses.missouristate.edu/KenVollmar/mars/ |
| Ripes | https://github.com/mortbopet/Ripes |
| VSRTL (Ripes's HDL library) | https://github.com/mortbopet/VSRTL |
| Venus (Berkeley) | https://github.com/ThaumicMekanism/venus |
| Jupiter | https://github.com/andrescv/Jupiter |
| Spike (RISC-V reference) | https://github.com/riscv-software-src/riscv-isa-sim |
| WebRISC-V | https://github.com/Mariotti94/WebRISC-V |
| rvemu (Rust, WASM) | https://github.com/d0iasm/rvemu |
| rrs (Rust, blog series) | https://gregchadwick.co.uk/blog/building-rrs-pt1/ |
| egui | https://github.com/emilk/egui |
| RISC-V ISA Specification | https://github.com/riscv/riscv-isa-manual |
| RISC-V ABI (syscall numbers) | https://github.com/riscv-non-isa/riscv-elf-psabi-doc |
