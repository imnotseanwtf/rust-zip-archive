use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::cli::Compression;

/// Create a zip archive at `output` containing every path in `inputs`.
pub fn create(
    output: &Path,
    inputs: &[PathBuf],
    compression: Compression,
    force: bool,
) -> Result<()> {
    if output.exists() && !force {
        bail!(
            "{} already exists (use --force to overwrite)",
            output.display()
        );
    }

    // Make sure the output directory exists (e.g. `-o out/backup.zip`).
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }
    }

    let file = File::create(output)
        .with_context(|| format!("creating archive {}", output.display()))?;
    let mut zip = ZipWriter::new(BufWriter::new(file));

    let method = compression.to_zip_method();
    let base_options = SimpleFileOptions::default()
        .compression_method(method)
        .large_file(true);

    // Resolve the archive's own path so we never add it to itself
    // (e.g. `rza create -o backup.zip .`).
    let self_path = output.canonicalize().ok();

    // Collect the full list of files first so we can show meaningful progress.
    let entries = collect_entries(inputs, self_path.as_deref())?;
    let bar = progress_bar(entries.len() as u64, "Archiving");

    for entry in &entries {
        bar.set_message(entry.name.clone());
        if entry.is_dir {
            zip.add_directory(&entry.name, base_options)
                .with_context(|| format!("adding directory {}", entry.name))?;
        } else {
            let mut options = base_options;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = fs::metadata(&entry.path) {
                    options = options.unix_permissions(meta.permissions().mode());
                }
            }
            zip.start_file(&entry.name, options)
                .with_context(|| format!("adding file {}", entry.name))?;
            let mut f = BufReader::new(
                File::open(&entry.path)
                    .with_context(|| format!("reading {}", entry.path.display()))?,
            );
            io::copy(&mut f, &mut zip)
                .with_context(|| format!("compressing {}", entry.name))?;
        }
        bar.inc(1);
    }

    zip.finish().context("finalizing archive")?;
    bar.finish_with_message(format!("Created {}", output.display()));
    Ok(())
}

/// Extract every entry of `archive` into `dest`.
pub fn extract(archive: &Path, dest: &Path, force: bool) -> Result<()> {
    let file = File::open(archive)
        .with_context(|| format!("opening archive {}", archive.display()))?;
    let mut zip = ZipArchive::new(BufReader::new(file))
        .with_context(|| format!("reading archive {}", archive.display()))?;

    fs::create_dir_all(dest)
        .with_context(|| format!("creating destination {}", dest.display()))?;

    let bar = progress_bar(zip.len() as u64, "Extracting");

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let raw_name = entry
            .enclosed_name()
            .with_context(|| format!("unsafe path in archive: {}", entry.name()))?;
        let outpath = dest.join(&raw_name);
        bar.set_message(raw_name.display().to_string());

        if entry.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            if outpath.exists() && !force {
                bail!(
                    "{} already exists (use --force to overwrite)",
                    outpath.display()
                );
            }
            let mut out = BufWriter::new(File::create(&outpath)?);
            io::copy(&mut entry, &mut out)?;

            // Restore the original Unix permissions (e.g. the executable bit).
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))
                        .with_context(|| {
                            format!("setting permissions on {}", outpath.display())
                        })?;
                }
            }
        }
        bar.inc(1);
    }

    bar.finish_with_message(format!("Extracted into {}", dest.display()));
    Ok(())
}

/// Print a human-readable table of the archive's contents.
pub fn list(archive: &Path) -> Result<()> {
    let file = File::open(archive)
        .with_context(|| format!("opening archive {}", archive.display()))?;
    let mut zip = ZipArchive::new(BufReader::new(file))
        .with_context(|| format!("reading archive {}", archive.display()))?;

    println!("{:>12}  {:>12}  {:>6}  Name", "Size", "Compressed", "Ratio");
    println!("{}", "-".repeat(60));

    let mut total_size = 0u64;
    let mut total_comp = 0u64;
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        let size = entry.size();
        let comp = entry.compressed_size();
        total_size += size;
        total_comp += comp;
        let ratio = if size == 0 { 0.0 } else { 100.0 * (1.0 - comp as f64 / size as f64) };
        println!(
            "{:>12}  {:>12}  {:>5.0}%  {}",
            size,
            comp,
            ratio,
            entry.name()
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
        zip.len()
    );
    Ok(())
}

struct Entry {
    /// Absolute/relative path on disk.
    path: PathBuf,
    /// Path stored inside the archive (always uses forward slashes).
    name: String,
    is_dir: bool,
}

/// Walk all inputs and build the list of archive entries with sanitized names.
/// `self_path`, when set, is the archive's own canonical path; matching entries
/// are skipped so the archive is never added to itself.
fn collect_entries(inputs: &[PathBuf], self_path: Option<&Path>) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for input in inputs {
        if !input.exists() {
            bail!("input does not exist: {}", input.display());
        }
        // The base parent determines how much of the path we strip so the
        // archive stores e.g. `mydir/file.txt` rather than the full path.
        let base = input.parent().unwrap_or_else(|| Path::new(""));
        for dent in WalkDir::new(input).into_iter() {
            let dent = dent?;
            let path = dent.path();

            // Don't add the archive we're currently writing to itself.
            if let Some(self_path) = self_path {
                if path.canonicalize().ok().as_deref() == Some(self_path) {
                    continue;
                }
            }

            let rel = path.strip_prefix(base).unwrap_or(path);
            let name = to_archive_name(rel);
            if name.is_empty() {
                continue;
            }
            entries.push(Entry {
                path: path.to_path_buf(),
                name,
                is_dir: dent.file_type().is_dir(),
            });
        }
    }
    Ok(entries)
}

/// Convert a relative path into a forward-slash archive name, dropping any
/// `.` / `..` components so we never write surprising paths into the zip.
fn to_archive_name(path: &Path) -> String {
    let mut parts = Vec::new();
    for comp in path.components() {
        if let Component::Normal(part) = comp {
            parts.push(part.to_string_lossy().into_owned());
        }
    }
    parts.join("/")
}

fn progress_bar(len: u64, verb: &str) -> ProgressBar {
    let bar = ProgressBar::new(len);
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
