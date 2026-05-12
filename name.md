# OARS — Name & Concept

## The Name

**OARS** — *Oxide Assembler and Runtime Simulator*

| Word | Meaning |
|---|---|
| **O**xide | Rust = iron oxide. The Rust language, made literal. |
| **A**ssembler | Two-pass RISC-V assembler, just like RARS/MARS |
| **R**untime | Execution engine, syscall handlers, hardware model |
| **S**imulator | Full visual simulation environment for students |

## Lineage

```
MARS  (MIPS Assembler and Runtime Simulator)  — the original
  └─ RARS  (RISC-V Assembler and Runtime Simulator)  — MARS forked for RISC-V
       └─ OARS  (Oxide Assembler and Runtime Simulator)  — RARS rebuilt in Rust, native binary
```

Each generation keeps the -ARS suffix and the same educational mission.

## The Concept

OARS is what RARS would be if it were rewritten today:

- **No JVM.** Students download one file and double-click. Windows, macOS, Linux.
- **Everything RARS has** — 35+ syscalls, full RV32F/D floating-point, pseudo-instructions, macros, backstepper, memory dumps, CLI mode.
- **What RARS is missing** — pipeline stage visualizer (inspired by Ripes), CPU variant selector (single-stage / 5-stage / no-forwarding / no-hazard), CLI telemetry for auto-grading, cache simulator tab.
- **Built in Rust** with egui — the same codebase compiles to a native binary on all platforms and optionally to WASM for a zero-install browser version.

## Names Considered

| Name | Expansion | Notes |
|---|---|---|
| **OARS** ✅ | Oxide Assembler and Runtime Simulator | Keeps -ARS lineage, rust = oxide, clean word |
| RIVET | RISC-V Interactive Visual Execution Tool | Rust/metal pun, RISC-V in the word |
| FARS | Ferrous Assembler and Runtime Simulator | Ferrous = iron, but sounds like "farce" |
| RAVE | RISC-V Assembler and Visual Environment | Modern, but loses the -ARS lineage |
| Ferris-V | Ferris (Rust mascot) + RISC-V | Fun, instantly recognisable to Rust devs |
| FORGE | — | Strong Rust/smithing theme, less clear as a simulator name |
