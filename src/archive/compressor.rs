//! Single-file compressors: gzip, bzip2, xz, zstd. Each holds exactly one file.

use anyhow::{bail, Context, Result};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use super::{EntryInfo, Progress};
use crate::archive::format::Format;

fn suffix(format: Format) -> &'static str {
    match format {
        Format::Gz => ".gz",
        Format::Bz2 => ".bz2",
        Format::Xz => ".xz",
        Format::Zst => ".zst",
        _ => "",
    }
}

/// The decompressed file name = archive file name minus the compressor suffix.
pub(crate) fn inner_name(archive: &Path, format: Format) -> String {
    let file = archive
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let suf = suffix(format);
    if !suf.is_empty() && file.to_lowercase().ends_with(suf) {
        file[..file.len() - suf.len()].to_string()
    } else {
        format!("{file}.out")
    }
}

pub(crate) fn create(
    output: &Path,
    input: &Path,
    format: Format,
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
            fs::create_dir_all(parent)?;
        }
    }
    let mut reader =
        BufReader::new(File::open(input).with_context(|| format!("reading {}", input.display()))?);
    let w = BufWriter::new(
        File::create(output).with_context(|| format!("creating {}", output.display()))?,
    );
    progress(Progress {
        current: 0,
        total: 1,
        message: input.display().to_string(),
    });
    match format {
        Format::Gz => {
            let mut enc = flate2::write::GzEncoder::new(w, flate2::Compression::default());
            io::copy(&mut reader, &mut enc)?;
            enc.finish()?;
        }
        Format::Bz2 => {
            let mut enc = bzip2::write::BzEncoder::new(w, bzip2::Compression::default());
            io::copy(&mut reader, &mut enc)?;
            enc.finish()?;
        }
        Format::Xz => {
            let mut enc = xz2::write::XzEncoder::new(w, 6);
            io::copy(&mut reader, &mut enc)?;
            enc.finish()?;
        }
        Format::Zst => {
            let mut enc = zstd::stream::write::Encoder::new(w, 0)?;
            io::copy(&mut reader, &mut enc)?;
            enc.finish()?;
        }
        other => bail!("compressor backend cannot create {:?}", other),
    }
    progress(Progress {
        current: 1,
        total: 1,
        message: "done".into(),
    });
    Ok(())
}

fn open_decoder(archive: &Path, format: Format) -> Result<Box<dyn Read>> {
    let r = BufReader::new(
        File::open(archive).with_context(|| format!("opening {}", archive.display()))?,
    );
    Ok(match format {
        Format::Gz => Box::new(flate2::read::GzDecoder::new(r)),
        Format::Bz2 => Box::new(bzip2::read::BzDecoder::new(r)),
        Format::Xz => Box::new(xz2::read::XzDecoder::new(r)),
        Format::Zst => Box::new(zstd::stream::read::Decoder::new(r)?),
        other => bail!("compressor backend cannot read {:?}", other),
    })
}

pub(crate) fn list(archive: &Path, format: Format) -> Result<Vec<EntryInfo>> {
    // gzip stores the uncompressed size in the trailing 4 bytes (ISIZE).
    let size = if format == Format::Gz {
        gzip_isize(archive).unwrap_or(0)
    } else {
        0
    };
    let compressed = fs::metadata(archive).map(|m| m.len()).unwrap_or(0);
    Ok(vec![EntryInfo {
        name: inner_name(archive, format),
        size,
        compressed,
        is_dir: false,
    }])
}

fn gzip_isize(archive: &Path) -> Option<u64> {
    use std::io::{Seek, SeekFrom};
    let mut f = File::open(archive).ok()?;
    f.seek(SeekFrom::End(-4)).ok()?;
    let mut b = [0u8; 4];
    f.read_exact(&mut b).ok()?;
    Some(u32::from_le_bytes(b) as u64)
}

pub(crate) fn extract(
    archive: &Path,
    dest: &Path,
    format: Format,
    force: bool,
    mut progress: impl FnMut(Progress),
) -> Result<()> {
    fs::create_dir_all(dest).with_context(|| format!("creating destination {}", dest.display()))?;
    let outpath = dest.join(inner_name(archive, format));
    if outpath.exists() && !force {
        bail!(
            "{} already exists (use --force to overwrite)",
            outpath.display()
        );
    }
    progress(Progress {
        current: 0,
        total: 1,
        message: outpath.display().to_string(),
    });
    let mut dec = open_decoder(archive, format)?;
    let mut out = BufWriter::new(File::create(&outpath)?);
    io::copy(&mut dec, &mut out)?;
    out.flush()?;
    progress(Progress {
        current: 1,
        total: 1,
        message: "done".into(),
    });
    Ok(())
}
