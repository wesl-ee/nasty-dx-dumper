use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod constants;
mod shape;
mod utils;

use commands::dump_all;

#[derive(Parser)]
#[command(name = "nasty_dx_dumper")]
#[command(about = "A Dokapon DX dumper and patcher")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    DumpAll { dir: PathBuf, dir_out: PathBuf },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::DumpAll { dir, dir_out } => dump_all(dir, dir_out),
    }
}
