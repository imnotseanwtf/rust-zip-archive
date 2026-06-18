use anyhow::{bail, Context, Result};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::cli::Compression;

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

/// Create a zip archive at `output` containing every path in `inputs`.
pub fn create(
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

    // Make sure the output directory exists (e.g. `-o out/backup.zip`).
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

    // Resolve the archive's own path so we never add it to itself
    // (e.g. `rza create -o backup.zip .`).
    let self_path = output.canonicalize().ok();

    // Collect the full list of files first so we can show meaningful progress.
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

/// Extract every entry of `archive` into `dest`.
pub fn extract(
    archive: &Path,
    dest: &Path,
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    extract_inner(archive, dest, None, force, progress)
}

/// Extract only the entries whose archive names appear in `names`.
pub fn extract_selected(
    archive: &Path,
    dest: &Path,
    names: &[String],
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    let set: std::collections::HashSet<&str> = names.iter().map(|s| s.as_str()).collect();
    extract_inner(archive, dest, Some(set), force, progress)
}

fn extract_inner(
    archive: &Path,
    dest: &Path,
    selected: Option<std::collections::HashSet<&str>>,
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

            // Restore the original Unix permissions (e.g. the executable bit).
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

/// Return metadata for every entry in the archive.
pub fn list(archive: &Path) -> Result<Vec<EntryInfo>> {
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

/// On non-Windows platforms the archive path is already a valid relative path.
#[cfg(not(windows))]
fn sanitize_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// On Windows, rewrite each path component so it is a legal filename:
/// reserved device names (CON, NUL, COM1…), illegal characters (`<>:"|?*`),
/// and trailing dots/spaces are all made safe by inserting an underscore.
#[cfg(windows)]
fn sanitize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(part) => out.push(sanitize_windows_name(&part.to_string_lossy())),
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Make a single path component safe to create on Windows.
#[cfg(windows)]
fn sanitize_windows_name(name: &str) -> String {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    // Replace characters Windows forbids in filenames.
    let mut cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();

    // The stem (text before the first dot) determines a reserved name clash.
    let stem = cleaned.split('.').next().unwrap_or("");
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        cleaned.insert(0, '_');
    }

    // Windows silently strips trailing dots and spaces; keep the name intact
    // by appending an underscore instead.
    if cleaned.ends_with('.') || cleaned.ends_with(' ') {
        cleaned.push('_');
    }

    if cleaned.is_empty() {
        cleaned.push('_');
    }
    cleaned
}
