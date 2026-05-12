use anyhow::Result;
use clap::{Args, Parser};
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
    /// Dump all register values after execution
    #[arg(long)]
    pub dump_registers: bool,

    /// Maximum number of instructions to execute (0 = unlimited)
    #[arg(long, default_value_t = 0)]
    pub max_steps: u64,

    /// Begin execution at label `main` instead of first instruction
    #[arg(long)]
    pub start_at_main: bool,

    /// Emit cycle/instruction count as JSON after execution (for auto-graders)
    #[arg(long)]
    pub telemetry: bool,
}

/// Headless (CLI) entry point — assembles and runs a .s file.
/// Implemented fully in Phase 1; this stub validates argument parsing.
pub fn run_headless(path: PathBuf, opts: &RunOpts) -> Result<()> {
    eprintln!("oars: headless runner not yet implemented (Phase 1)");
    eprintln!("  file: {}", path.display());
    eprintln!("  opts: {:?}", opts);
    Ok(())
}
