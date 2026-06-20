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

/// Output archive path for a quick-compress action: the item's parent dir +
/// base name + `ext`. Base name is the directory name for a folder, or the file
/// stem (last extension removed) for a file. `ext` includes the leading dot.
fn archive_output(path: &Path, ext: &str) -> PathBuf {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let base = if path.is_dir() {
        path.file_name().map(|s| s.to_string_lossy().to_string())
    } else {
        path.file_stem().map(|s| s.to_string_lossy().to_string())
    }
    .unwrap_or_default();
    parent.join(format!("{base}{ext}"))
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

        Command::Test { archive } => {
            let bar = make_bar("Testing");
            rust_zip_archive::archive::test(&archive, |p: Progress| {
                bar.set_length(p.total);
                bar.set_position(p.current);
                bar.set_message(p.message);
            })?;
            bar.finish_with_message(format!("OK — {} is valid", archive.display()));
        }

        Command::CompressZip { path } => {
            let output = archive_output(&path, ".zip");
            let bar = make_bar("Archiving");
            rust_zip_archive::archive::create(
                &output,
                std::slice::from_ref(&path),
                rust_zip_archive::cli::Compression::Deflate,
                false,
                |p: Progress| {
                    bar.set_length(p.total);
                    bar.set_position(p.current);
                    bar.set_message(p.message);
                },
            )?;
            bar.finish_with_message(format!("Created {}", output.display()));
        }

        Command::CompressTargz { path } => {
            let output = archive_output(&path, ".tar.gz");
            let bar = make_bar("Archiving");
            rust_zip_archive::archive::create(
                &output,
                std::slice::from_ref(&path),
                rust_zip_archive::cli::Compression::Deflate,
                false,
                |p: Progress| {
                    bar.set_length(p.total);
                    bar.set_position(p.current);
                    bar.set_message(p.message);
                },
            )?;
            bar.finish_with_message(format!("Created {}", output.display()));
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

    use super::archive_output;

    #[test]
    fn output_strips_file_extension() {
        assert_eq!(
            archive_output(Path::new("/x/report.docx"), ".zip"),
            PathBuf::from("/x/report.zip")
        );
    }

    #[test]
    fn output_no_extension_appends() {
        assert_eq!(
            archive_output(Path::new("/x/notes"), ".zip"),
            PathBuf::from("/x/notes.zip")
        );
    }

    #[test]
    fn output_two_part_ext() {
        assert_eq!(
            archive_output(Path::new("/x/report.docx"), ".tar.gz"),
            PathBuf::from("/x/report.tar.gz")
        );
    }

    #[test]
    fn output_folder_uses_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir(&photos).unwrap();
        assert_eq!(
            archive_output(&photos, ".zip"),
            dir.path().join("photos.zip")
        );
    }

    #[test]
    fn output_empty_parent_uses_dot() {
        assert_eq!(
            archive_output(Path::new("report.docx"), ".zip"),
            Path::new(".").join("report.zip")
        );
    }
}
