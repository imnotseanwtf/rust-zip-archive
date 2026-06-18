use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};

use rust_zip_archive::archive::{self, Progress};
use rust_zip_archive::cli::{Cli, Command};

fn make_bar(verb: &str) -> ProgressBar {
    let bar = ProgressBar::new(0);
    bar.set_style(
        ProgressStyle::with_template(
            "{prefix:.bold.dim} [{bar:30.cyan/blue}] {pos}/{len} {wide_msg}",
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    bar.set_prefix(verb.to_string());
    bar
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Create {
            output,
            inputs,
            method,
            force,
        } => {
            let bar = make_bar("Archiving");
            archive::create(&output, &inputs, method, force, |p: Progress| {
                bar.set_length(p.total);
                bar.set_position(p.current);
                bar.set_message(p.message);
            })?;
            bar.finish_with_message(format!("Created {}", output.display()));
        }

        Command::Extract {
            archive,
            dest,
            force,
        } => {
            let bar = make_bar("Extracting");
            rust_zip_archive::archive::extract(&archive, &dest, force, |p: Progress| {
                bar.set_length(p.total);
                bar.set_position(p.current);
                bar.set_message(p.message);
            })?;
            bar.finish_with_message(format!("Extracted into {}", dest.display()));
        }

        Command::List { archive } => {
            let entries = rust_zip_archive::archive::list(&archive)?;
            println!("{:>12}  {:>12}  {:>6}  Name", "Size", "Compressed", "Ratio");
            println!("{}", "-".repeat(60));
            let mut total_size = 0u64;
            let mut total_comp = 0u64;
            for e in &entries {
                total_size += e.size;
                total_comp += e.compressed;
                let ratio = if e.size == 0 {
                    0.0
                } else {
                    100.0 * (1.0 - e.compressed as f64 / e.size as f64)
                };
                println!(
                    "{:>12}  {:>12}  {:>5.0}%  {}",
                    e.size, e.compressed, ratio, e.name
                );
            }
            println!("{}", "-".repeat(60));
            let total_ratio = if total_size == 0 {
                0.0
            } else {
                100.0 * (1.0 - total_comp as f64 / total_size as f64)
            };
            println!(
                "{:>12}  {:>12}  {:>5.0}%  {} file(s)",
                total_size,
                total_comp,
                total_ratio,
                entries.len()
            );
        }
    }

    Ok(())
}
