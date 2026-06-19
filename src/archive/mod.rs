//! Multi-format archive API. Detects the format and dispatches to a backend.

mod compressor;
pub mod format;
mod tar;
mod zip;

use anyhow::{bail, Result};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

use crate::cli::Compression;
use format::Format;

/// Progress update emitted by long-running operations.
pub struct Progress {
    pub current: u64,
    pub total: u64,
    pub message: String,
}

/// Metadata about one entry in an archive (used by `list`).
pub struct EntryInfo {
    pub name: String,
    pub size: u64,
    pub compressed: u64,
    pub is_dir: bool,
}

/// A file/dir staged for archiving.
pub(crate) struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

pub fn create(
    output: &Path,
    inputs: &[PathBuf],
    compression: Compression,
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    let format = format::detect_for_write(output)?;
    match format {
        Format::Zip => zip::create(output, inputs, compression, force, progress),
        Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst => {
            tar::create(output, inputs, format, force, progress)
        }
        Format::Gz | Format::Bz2 | Format::Xz | Format::Zst => {
            if inputs.len() != 1 || inputs[0].is_dir() {
                bail!(
                    "{:?} compresses a single file; use a .tar.* format to archive multiple files or a directory",
                    format
                );
            }
            compressor::create(output, &inputs[0], format, force, progress)
        }
        other => bail!("creating {:?} archives is not supported yet", other),
    }
}

pub fn list(archive: &Path) -> Result<Vec<EntryInfo>> {
    let format = format::detect_for_read(archive)?;
    match format {
        Format::Zip => zip::list(archive),
        Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst => {
            tar::list(archive, format)
        }
        Format::Gz | Format::Bz2 | Format::Xz | Format::Zst => compressor::list(archive, format),
        other => bail!("listing {:?} archives is not supported yet", other),
    }
}

pub fn extract(
    archive: &Path,
    dest: &Path,
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    let format = format::detect_for_read(archive)?;
    match format {
        Format::Zip => zip::extract(archive, dest, force, progress),
        Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst => {
            tar::extract(archive, dest, format, force, progress)
        }
        Format::Gz | Format::Bz2 | Format::Xz | Format::Zst => {
            compressor::extract(archive, dest, format, force, progress)
        }
        other => bail!("extracting {:?} archives is not supported yet", other),
    }
}

pub fn extract_selected(
    archive: &Path,
    dest: &Path,
    names: &[String],
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    let format = format::detect_for_read(archive)?;
    match format {
        Format::Zip => zip::extract_selected(archive, dest, names, force, progress),
        Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst => {
            tar::extract_selected(archive, dest, names, format, force, progress)
        }
        Format::Gz | Format::Bz2 | Format::Xz | Format::Zst => {
            compressor::extract(archive, dest, format, force, progress)
        }
        other => bail!("extracting {:?} archives is not supported yet", other),
    }
}

/// Walk all inputs into archive entries with sanitized forward-slash names.
/// `self_path`, when set, is the archive's own canonical path; matching entries
/// are skipped so the archive is never added to itself.
pub(crate) fn collect_entries(inputs: &[PathBuf], self_path: Option<&Path>) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for input in inputs {
        if !input.exists() {
            bail!("input does not exist: {}", input.display());
        }
        let base = input.parent().unwrap_or_else(|| Path::new(""));
        for dent in WalkDir::new(input).into_iter() {
            let dent = dent?;
            let path = dent.path();
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

/// Convert a relative path to a forward-slash archive name, dropping `.`/`..`.
pub(crate) fn to_archive_name(path: &Path) -> String {
    let mut parts = Vec::new();
    for comp in path.components() {
        if let Component::Normal(part) = comp {
            parts.push(part.to_string_lossy().into_owned());
        }
    }
    parts.join("/")
}

#[cfg(not(windows))]
pub(crate) fn sanitize_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(windows)]
pub(crate) fn sanitize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(part) => out.push(sanitize_windows_name(&part.to_string_lossy())),
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(windows)]
fn sanitize_windows_name(name: &str) -> String {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let mut cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    let stem = cleaned.split('.').next().unwrap_or("");
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        cleaned.insert(0, '_');
    }
    if cleaned.ends_with('.') || cleaned.ends_with(' ') {
        cleaned.push('_');
    }
    if cleaned.is_empty() {
        cleaned.push('_');
    }
    cleaned
}
