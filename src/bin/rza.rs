use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};

use rust_zip_archive::archive::{self, Progress};
use rust_zip_archive::cli::{Cli, Command};

/// Destination for "Extract Here": the archive's own directory.
fn extract_here_dir(archive: &Path) -> PathBuf {
    archive
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

/// Destination for "Extract to name\": parent dir + the file name with a
/// recognized archive/compressor suffix removed (fallback `<name>_extracted`).
fn extract_to_dir(archive: &Path) -> PathBuf {
    const SUFFIXES: &[&str] = &[
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tgz", ".tbz2", ".txz", ".tzst", ".tar",
        ".zip", ".7z", ".rar", ".gz", ".bz2", ".xz", ".zst",
    ];
    let parent = archive
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let fname = archive
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let lower = fname.to_lowercase();
    let stem = SUFFIXES
        .iter()
        .find(|s| lower.ends_with(*s))
        .map(|s| fname[..fname.len() - s.len()].to_string())
        .unwrap_or_else(|| format!("{fname}_extracted"));
    parent.join(stem)
}

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

        Command::ExtractHere { archive } => {
            let dest = extract_here_dir(&archive);
            let bar = make_bar("Extracting");
            rust_zip_archive::archive::extract(&archive, &dest, false, |p: Progress| {
                bar.set_length(p.total);
                bar.set_position(p.current);
                bar.set_message(p.message);
            })?;
            bar.finish_with_message(format!("Extracted into {}", dest.display()));
        }

        Command::ExtractTo { archive } => {
            let dest = extract_to_dir(&archive);
            let bar = make_bar("Extracting");
            rust_zip_archive::archive::extract(&archive, &dest, false, |p: Progress| {
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

#[cfg(test)]
mod tests {
    use super::{extract_here_dir, extract_to_dir};
    use std::path::{Path, PathBuf};

    #[test]
    fn here_dir_is_parent() {
        assert_eq!(
            extract_here_dir(Path::new("/tmp/a/b.zip")),
            PathBuf::from("/tmp/a")
        );
    }

    #[test]
    fn to_dir_strips_known_suffixes() {
        assert_eq!(
            extract_to_dir(Path::new("/tmp/a/b.zip")),
            PathBuf::from("/tmp/a/b")
        );
        assert_eq!(
            extract_to_dir(Path::new("/tmp/a/b.tar.gz")),
            PathBuf::from("/tmp/a/b")
        );
        assert_eq!(
            extract_to_dir(Path::new("/tmp/a/b.tgz")),
            PathBuf::from("/tmp/a/b")
        );
    }

    #[test]
    fn to_dir_fallback_when_no_known_suffix() {
        assert_eq!(
            extract_to_dir(Path::new("/tmp/a/weird.bin")),
            PathBuf::from("/tmp/a/weird.bin_extracted")
        );
    }
}
