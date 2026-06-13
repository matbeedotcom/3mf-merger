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

        #[arg(long, help = "Print merged printer preset settings to terminal")]
        printer_preset: bool,

        #[arg(long, help = "Print merged filament colour presets to terminal")]
        color_presets: bool,

        #[arg(
            long,
            help = "Only keep first input's printer settings (default: true)"
        )]
        keep_first_printer: bool,

        #[arg(
            long,
            help = "Only keep first input's filament settings (default: true)"
        )]
        keep_first_filament: bool,

        #[arg(long, help = "Merge filament settings from all inputs")]
        merge_filament: bool,

        #[arg(long, help = "Merge printer settings from all inputs")]
        merge_printer: bool,

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
            printer_preset,
            color_presets,
            keep_first_printer,
            keep_first_filament,
            merge_filament,
            merge_printer,
            inputs,
        } => {
            three_mf_merger::merge_files(
                &inputs,
                &output,
                force,
                printer_preset,
                color_presets,
                keep_first_printer,
                keep_first_filament,
                merge_filament,
                merge_printer,
            )?;
            println!("merged {} files into {}", inputs.len(), output.display());
        }
    }

    Ok(())
}
