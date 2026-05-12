use crate::assembler::{codegen, parser};
use crate::simulator::engine::{self, CpuState};
use anyhow::{Context, Result};
use clap::{Args, Parser};
use std::io::{self, BufReader};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "oars", about = "Oxide Assembler and Runtime Simulator")]
pub struct Cli {
    /// RISC-V assembly source file to assemble and run
    pub file: Option<PathBuf>,

    #[command(flatten)]
    pub opts: RunOpts,
}

#[derive(Args, Debug, Default)]
pub struct RunOpts {
    /// Dump integer register values after execution
    #[arg(long)]
    pub dump_registers: bool,

    /// Dump floating-point register values after execution
    #[arg(long)]
    pub dump_fp_registers: bool,

    /// Maximum number of instructions to execute (0 = unlimited)
    #[arg(long, default_value_t = 0)]
    pub max_steps: u64,

    /// Begin execution at label `main` instead of first instruction
    #[arg(long)]
    pub start_at_main: bool,

    /// Emit instruction count + exit code as JSON (for auto-graders)
    #[arg(long)]
    pub telemetry: bool,
}

pub fn run_headless(path: PathBuf, opts: &RunOpts) -> Result<()> {
    let src = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;

    // Parse
    let stmts = parser::parse(&src)
        .with_context(|| format!("parse error in {}", path.display()))?;

    // Assemble into a fresh memory image
    let mut cpu = CpuState::new(crate::hardware::memory::TEXT_BASE);
    let out = codegen::assemble(&stmts, &mut cpu.mem)
        .with_context(|| format!("assembly error in {}", path.display()))?;

    cpu.pc = out.entry;

    // Run
    let mut stdout = io::stdout();
    let stdin_raw = io::stdin();
    let mut stdin = BufReader::new(stdin_raw.lock());

    let telem = engine::run(&mut cpu, opts, &mut stdout, &mut stdin)?;

    // Post-run output
    if opts.dump_registers {
        eprintln!("\n── Integer Registers ───────────────────────────────────");
        for (name, val) in cpu.regs.dump() {
            eprintln!("  {name} = {val:#010x}  ({})", val as i32);
        }
    }

    if opts.dump_fp_registers {
        eprintln!("\n── FP Registers ────────────────────────────────────────");
        for (name, val) in cpu.fp.dump() {
            eprintln!("  {name} = {val:.6}");
        }
    }

    if opts.telemetry {
        let json = serde_json::to_string_pretty(&telem)?;
        eprintln!("{json}");
    }

    Ok(())
}
