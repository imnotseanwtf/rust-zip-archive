//! tar backend, with optional gzip/bzip2/xz/zstd compression layers.

use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use super::{collect_entries, sanitize_path, EntryInfo, Progress};
use crate::archive::format::Format;

pub(crate) fn create(
    output: &Path,
    inputs: &[PathBuf],
    format: Format,
    force: bool,
    progress: impl FnMut(Progress),
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
    let w = BufWriter::new(file);
    let self_path = output.canonicalize().ok();
    let entries = collect_entries(inputs, self_path.as_deref())?;

    // Build the tar over the right (possibly compressing) writer, then finalize
    // the compression layer.
    match format {
        Format::Tar => {
            let mut b = tar::Builder::new(w);
            write_entries(&mut b, &entries, progress)?;
            b.into_inner()?.flush()?;
        }
        Format::TarGz => {
            let enc = flate2::write::GzEncoder::new(w, flate2::Compression::default());
            let mut b = tar::Builder::new(enc);
            write_entries(&mut b, &entries, progress)?;
            b.into_inner()?.finish()?;
        }
        Format::TarBz2 => {
            let enc = bzip2::write::BzEncoder::new(w, bzip2::Compression::default());
            let mut b = tar::Builder::new(enc);
            write_entries(&mut b, &entries, progress)?;
            b.into_inner()?.finish()?;
        }
        Format::TarXz => {
            let enc = xz2::write::XzEncoder::new(w, 6);
            let mut b = tar::Builder::new(enc);
            write_entries(&mut b, &entries, progress)?;
            b.into_inner()?.finish()?;
        }
        Format::TarZst => {
            let enc = zstd::stream::write::Encoder::new(w, 0)?;
            let mut b = tar::Builder::new(enc);
            write_entries(&mut b, &entries, progress)?;
            b.into_inner()?.finish()?; // propagate frame-flush errors
        }
        other => bail!("tar backend cannot create {:?}", other),
    }
    Ok(())
}

fn write_entries<W: Write>(
    builder: &mut tar::Builder<W>,
    entries: &[super::Entry],
    mut progress: impl FnMut(Progress),
) -> Result<()> {
    let total = entries.len() as u64;
    for (i, entry) in entries.iter().enumerate() {
        progress(Progress {
            current: i as u64,
            total,
            message: entry.name.clone(),
        });
        // append_path_with_name reads metadata (mode, mtime) and contents from
        // disk and stores them under the archive-relative name.
        builder
            .append_path_with_name(&entry.path, &entry.name)
            .with_context(|| format!("adding {}", entry.name))?;
    }
    progress(Progress {
        current: total,
        total,
        message: "done".into(),
    });
    Ok(())
}

/// Open the archive and hand a tar reader to `f`, decompressing as needed.
fn with_reader<T>(
    archive: &Path,
    format: Format,
    f: impl FnOnce(tar::Archive<Box<dyn Read>>) -> Result<T>,
) -> Result<T> {
    let file =
        File::open(archive).with_context(|| format!("opening archive {}", archive.display()))?;
    let r = BufReader::new(file);
    let inner: Box<dyn Read> = match format {
        Format::Tar => Box::new(r),
        Format::TarGz => Box::new(flate2::read::GzDecoder::new(r)),
        Format::TarBz2 => Box::new(bzip2::read::BzDecoder::new(r)),
        Format::TarXz => Box::new(xz2::read::XzDecoder::new(r)),
        Format::TarZst => Box::new(zstd::stream::read::Decoder::new(r)?),
        other => bail!("tar backend cannot read {:?}", other),
    };
    f(tar::Archive::new(inner))
}

pub(crate) fn test(
    archive: &Path,
    format: Format,
    mut progress: impl FnMut(Progress),
) -> Result<()> {
    with_reader(archive, format, |mut ar| {
        let mut idx = 0u64;
        for entry in ar.entries()? {
            let mut entry = entry?;
            let name = entry.path()?.to_string_lossy().to_string();
            progress(Progress {
                current: idx,
                total: 0,
                message: name.clone(),
            });
            idx += 1;
            std::io::copy(&mut entry, &mut std::io::sink())
                .with_context(|| format!("verifying {name}"))?;
        }
        progress(Progress {
            current: idx,
            total: idx,
            message: "ok".into(),
        });
        Ok(())
    })
}

pub(crate) fn list(archive: &Path, format: Format) -> Result<Vec<EntryInfo>> {
    with_reader(archive, format, |mut ar| {
        let mut out = Vec::new();
        for entry in ar.entries()? {
            let entry = entry?;
            let header = entry.header();
            let is_dir = header.entry_type().is_dir();
            let size = header.size().unwrap_or(0);
            let name = entry.path()?.to_string_lossy().to_string();
            out.push(EntryInfo {
                name,
                size,
                compressed: size,
                is_dir,
            });
        }
        Ok(out)
    })
}

pub(crate) fn extract(
    archive: &Path,
    dest: &Path,
    format: Format,
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    extract_impl(archive, dest, None, format, force, progress)
}

pub(crate) fn extract_selected(
    archive: &Path,
    dest: &Path,
    names: &[String],
    format: Format,
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    let set: HashSet<String> = names.iter().cloned().collect();
    extract_impl(archive, dest, Some(set), format, force, progress)
}

fn extract_impl(
    archive: &Path,
    dest: &Path,
    selected: Option<HashSet<String>>,
    format: Format,
    force: bool,
    mut progress: impl FnMut(Progress),
) -> Result<()> {
    fs::create_dir_all(dest).with_context(|| format!("creating destination {}", dest.display()))?;
    with_reader(archive, format, |mut ar| {
        let mut idx = 0u64;
        for entry in ar.entries()? {
            let mut entry = entry?;
            let raw = entry.path()?.to_path_buf();
            let raw_str = raw.to_string_lossy().to_string();
            if let Some(set) = &selected {
                if !set.contains(&raw_str) {
                    continue;
                }
            }
            // Path safety: reject absolute / parent-dir escapes, then sanitize.
            if raw.is_absolute()
                || raw
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                bail!("unsafe path in archive: {}", raw_str);
            }
            let safe = sanitize_path(&raw);
            let outpath = dest.join(&safe);
            progress(Progress {
                current: idx,
                total: 0,
                message: safe.display().to_string(),
            });
            idx += 1;

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&outpath)?;
                continue;
            }
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
            out.flush()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mode) = entry.header().mode() {
                    let _ = fs::set_permissions(&outpath, fs::Permissions::from_mode(mode));
                }
            }
        }
        progress(Progress {
            current: idx,
            total: idx,
            message: "done".into(),
        });
        Ok(())
    })
}
