use anyhow::Result;
use clap::Parser;
use oars::{cli, gui};

fn main() -> Result<()> {
    let args = cli::Cli::parse();

    match args.file {
        Some(path) => cli::run_headless(path, &args.opts),
        None => gui::launch(),
    }
}
