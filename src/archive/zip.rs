//! ZIP backend.

use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use super::{collect_entries, sanitize_path, EntryInfo, Progress};
use crate::cli::Compression;

pub(crate) fn create(
    output: &Path,
    inputs: &[PathBuf],
    compression: Compression,
    force: bool,
    mut progress: impl FnMut(Progress),
) -> Result<()> {
    if output.exists() && !force {
        bail!(
            "{} already exists (use --force to overwrite)",
            output.display()
        );
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }
    }
    let file =
        File::create(output).with_context(|| format!("creating archive {}", output.display()))?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let method = compression.to_zip_method();
    let base_options = SimpleFileOptions::default()
        .compression_method(method)
        .large_file(true);
    let self_path = output.canonicalize().ok();
    let entries = collect_entries(inputs, self_path.as_deref())?;
    let total = entries.len() as u64;
    for (i, entry) in entries.iter().enumerate() {
        progress(Progress {
            current: i as u64,
            total,
            message: entry.name.clone(),
        });
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
            io::copy(&mut f, &mut zip).with_context(|| format!("compressing {}", entry.name))?;
        }
    }
    zip.finish().context("finalizing archive")?;
    progress(Progress {
        current: total,
        total,
        message: "done".into(),
    });
    Ok(())
}

pub(crate) fn list(archive: &Path) -> Result<Vec<EntryInfo>> {
    let file =
        File::open(archive).with_context(|| format!("opening archive {}", archive.display()))?;
    let mut zip = ZipArchive::new(BufReader::new(file))
        .with_context(|| format!("reading archive {}", archive.display()))?;
    let mut entries = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        entries.push(EntryInfo {
            name: entry.name().to_string(),
            size: entry.size(),
            compressed: entry.compressed_size(),
            is_dir: entry.is_dir(),
        });
    }
    Ok(entries)
}

pub(crate) fn extract(
    archive: &Path,
    dest: &Path,
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    extract_inner(archive, dest, None, force, progress)
}

pub(crate) fn extract_selected(
    archive: &Path,
    dest: &Path,
    names: &[String],
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    let set: HashSet<&str> = names.iter().map(|s| s.as_str()).collect();
    extract_inner(archive, dest, Some(set), force, progress)
}

fn extract_inner(
    archive: &Path,
    dest: &Path,
    selected: Option<HashSet<&str>>,
    force: bool,
    mut progress: impl FnMut(Progress),
) -> Result<()> {
    let file =
        File::open(archive).with_context(|| format!("opening archive {}", archive.display()))?;
    let mut zip = ZipArchive::new(BufReader::new(file))
        .with_context(|| format!("reading archive {}", archive.display()))?;
    fs::create_dir_all(dest).with_context(|| format!("creating destination {}", dest.display()))?;
    let total = zip.len() as u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if let Some(set) = &selected {
            if !set.contains(entry.name()) {
                continue;
            }
        }
        let raw_name = entry
            .enclosed_name()
            .with_context(|| format!("unsafe path in archive: {}", entry.name()))?;
        let safe_name = sanitize_path(&raw_name);
        let outpath = dest.join(&safe_name);
        progress(Progress {
            current: i as u64,
            total,
            message: safe_name.display().to_string(),
        });
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
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))
                        .with_context(|| format!("setting permissions on {}", outpath.display()))?;
                }
            }
        }
    }
    progress(Progress {
        current: total,
        total,
        message: "done".into(),
    });
    Ok(())
}
