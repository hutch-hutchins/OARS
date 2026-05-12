mod assembler;
mod cli;
mod gui;
mod hardware;
mod isa;
mod simulator;
mod util;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

fn main() -> Result<()> {
    let args = Cli::parse();

    match args.file {
        Some(path) => cli::run_headless(path, &args.opts),
        None => {
            // Phase 3: launch GUI
            eprintln!("GUI not yet implemented — provide a .s file to run headless.");
            eprintln!("Usage: oars <file.s>");
            std::process::exit(1);
        }
    }
}
