use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod constants;
mod shape;
mod utils;

use commands::{dump_all, patch_all};

#[derive(Parser)]
#[command(name = "nasty_dx_dumper")]
#[command(about = "A Dokapon DX dumper and patcher")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    DumpAll { dir: PathBuf },
    PatchAll { dir: PathBuf, out_dir: PathBuf },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::DumpAll { dir } => dump_all(dir),
        Commands::PatchAll { dir, out_dir } => patch_all(dir, out_dir),
    }
}
