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
        None => gui::launch(),
    }
}
