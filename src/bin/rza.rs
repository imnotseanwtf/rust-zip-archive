use anyhow::Result;
use clap::Parser;

use rust_zip_archive::archive;
use rust_zip_archive::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Create {
            output,
            inputs,
            method,
            force,
        } => archive::create(&output, &inputs, method, force)?,

        Command::Extract {
            archive,
            dest,
            force,
        } => archive::extract(&archive, &dest, force)?,

        Command::List { archive } => archive::list(&archive)?,
    }

    Ok(())
}
