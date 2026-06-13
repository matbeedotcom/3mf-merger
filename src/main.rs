use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "3mf-merger")]
#[command(about = "Merge two or more 3MF packages into one 3MF package.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Merge {
        #[arg(short, long)]
        output: PathBuf,

        #[arg(long)]
        force: bool,

        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Merge {
            output,
            force,
            inputs,
        } => {
            three_mf_merger::merge_files(&inputs, &output, force)?;
            println!("merged {} files into {}", inputs.len(), output.display());
        }
    }

    Ok(())
}
