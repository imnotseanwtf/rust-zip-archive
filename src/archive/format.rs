//! Archive format identification by magic bytes (for reading) and by file
//! extension (for writing).

use anyhow::{bail, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    Gz,
    Bz2,
    Xz,
    Zst,
    SevenZ,
    Rar,
}

/// Lower-cased extension test that understands the double `.tar.*` endings.
fn ext_matches(path: &Path, suffix: &str) -> bool {
    path.to_string_lossy().to_lowercase().ends_with(suffix)
}

/// Determine the format to write from the output path's extension.
pub fn detect_for_write(path: &Path) -> Result<Format> {
    let p = path;
    let f = if ext_matches(p, ".tar.gz") || ext_matches(p, ".tgz") {
        Format::TarGz
    } else if ext_matches(p, ".tar.bz2") || ext_matches(p, ".tbz2") {
        Format::TarBz2
    } else if ext_matches(p, ".tar.xz") || ext_matches(p, ".txz") {
        Format::TarXz
    } else if ext_matches(p, ".tar.zst") || ext_matches(p, ".tzst") {
        Format::TarZst
    } else if ext_matches(p, ".tar") {
        Format::Tar
    } else if ext_matches(p, ".zip") {
        Format::Zip
    } else if ext_matches(p, ".7z") {
        Format::SevenZ
    } else if ext_matches(p, ".gz") {
        Format::Gz
    } else if ext_matches(p, ".bz2") {
        Format::Bz2
    } else if ext_matches(p, ".xz") {
        Format::Xz
    } else if ext_matches(p, ".zst") {
        Format::Zst
    } else if ext_matches(p, ".rar") {
        Format::Rar
    } else {
        bail!(
            "cannot determine archive format from output name: {}",
            p.display()
        );
    };
    Ok(f)
}

/// Determine the format to read by sniffing magic bytes, falling back to the
/// extension (which also disambiguates tarballs from single-file compressors).
pub fn detect_for_read(path: &Path) -> Result<Format> {
    let mut buf = [0u8; 6];
    let n = {
        let mut f =
            File::open(path).map_err(|e| anyhow::anyhow!("opening {}: {e}", path.display()))?;
        f.read(&mut buf).unwrap_or(0)
    };
    let b = &buf[..n];

    let starts = |sig: &[u8]| b.len() >= sig.len() && &b[..sig.len()] == sig;

    // Compression/container is identified by magic; tar-vs-single for the
    // gz/bz2/xz/zst family is then refined by extension.
    if starts(&[0x50, 0x4B, 0x03, 0x04]) || starts(&[0x50, 0x4B, 0x05, 0x06]) {
        return Ok(Format::Zip);
    }
    if starts(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Ok(Format::SevenZ);
    }
    if starts(&[0x52, 0x61, 0x72, 0x21]) {
        return Ok(Format::Rar);
    }
    if starts(&[0x1F, 0x8B]) {
        return Ok(
            if ext_matches(path, ".tar.gz") || ext_matches(path, ".tgz") {
                Format::TarGz
            } else {
                Format::Gz
            },
        );
    }
    if starts(&[0xFD, b'7', b'z', b'X', b'Z', 0x00]) {
        return Ok(
            if ext_matches(path, ".tar.xz") || ext_matches(path, ".txz") {
                Format::TarXz
            } else {
                Format::Xz
            },
        );
    }
    if starts(&[0x28, 0xB5, 0x2F, 0xFD]) {
        return Ok(
            if ext_matches(path, ".tar.zst") || ext_matches(path, ".tzst") {
                Format::TarZst
            } else {
                Format::Zst
            },
        );
    }
    if starts(&[0x42, 0x5A, 0x68]) {
        return Ok(
            if ext_matches(path, ".tar.bz2") || ext_matches(path, ".tbz2") {
                Format::TarBz2
            } else {
                Format::Bz2
            },
        );
    }
    // Uncompressed tar: "ustar" appears at offset 257, beyond our small buffer,
    // so fall back to the extension for plain .tar.
    if ext_matches(path, ".tar") {
        return Ok(Format::Tar);
    }
    bail!(
        "unrecognized or unsupported archive format: {}",
        path.display()
    );
}
