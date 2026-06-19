# Multi-Format Support — Plan A (Core: tar + tarballs + single-file compressors)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add tar, .tar.gz/.bz2/.xz/.zst, and standalone .gz/.bz2/.xz/.zst support to `rza` behind a format-abstraction layer, so create/list/extract auto-route by format. (7z and rar are later plans B and C.)

**Architecture:** Refactor the single `src/archive.rs` into an `src/archive/` module tree: `mod.rs` holds the public API + shared helpers + a `match Format` dispatcher; `format.rs` detects the format (magic bytes + extension); `zip.rs`, `tar.rs`, `compressor.rs` are isolated backends. `Progress`/`EntryInfo` stay the shared currency, so the CLI and GUI gain every format with no call-site changes.

**Tech Stack:** Rust 2021; existing `zip`/`walkdir`/`anyhow`; add `tar`, `flate2`, `bzip2`, `xz2`, `zstd`; dev `tempfile`.

## Global Constraints

- Edition 2021; must build/run on Linux, macOS, Windows.
- `cargo build` (no features) stays CLI-only; the `gui` feature stays opt-in.
- Existing CLI behavior for `.zip` (commands, flags, list-table output, progress bars) must not change.
- `--method` (Store/Deflate/Bzip2/Zstd) applies to `.zip` only.
- Zip-slip protection and Windows reserved-name handling preserved for every multi-entry backend's extraction.
- Single-file compressors (.gz/.bz2/.xz/.zst) hold exactly one file.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` clean.
- DRY: shared walking/naming/sanitizing helpers live once in `mod.rs`; backends reuse them.

---

### Task 1: Refactor `archive.rs` → `archive/` module tree + format detection + dispatch

Split the ZIP-only file into a module tree with shared helpers, a `Format` enum with detection, and a dispatcher. Zip is wired through the dispatcher; not-yet-implemented formats return a clear "not supported yet" error. No behavior change for zip.

**Files:**
- Create: `src/archive/mod.rs` (public API + shared helpers + dispatch)
- Create: `src/archive/format.rs` (Format enum + detection)
- Create: `src/archive/zip.rs` (moved ZIP logic)
- Delete: `src/archive.rs`
- Modify: `src/lib.rs` (module path unchanged: `pub mod archive;` still works for a dir module)
- Test: `tests/roundtrip.rs` (existing tests must keep passing), `tests/detection.rs` (new)

**Interfaces:**
- Produces (public, in `mod.rs`, unchanged signatures):
  - `pub struct Progress { pub current: u64, pub total: u64, pub message: String }`
  - `pub struct EntryInfo { pub name: String, pub size: u64, pub compressed: u64, pub is_dir: bool }`
  - `pub fn create(output: &Path, inputs: &[PathBuf], compression: crate::cli::Compression, force: bool, progress: impl FnMut(Progress)) -> Result<()>`
  - `pub fn list(archive: &Path) -> Result<Vec<EntryInfo>>`
  - `pub fn extract(archive: &Path, dest: &Path, force: bool, progress: impl FnMut(Progress)) -> Result<()>`
  - `pub fn extract_selected(archive: &Path, dest: &Path, names: &[String], force: bool, progress: impl FnMut(Progress)) -> Result<()>`
- Produces (crate-internal helpers in `mod.rs`, reused by backends):
  - `pub(crate) struct Entry { pub path: PathBuf, pub name: String, pub is_dir: bool }`
  - `pub(crate) fn collect_entries(inputs: &[PathBuf], self_path: Option<&Path>) -> Result<Vec<Entry>>`
  - `pub(crate) fn to_archive_name(path: &Path) -> String`
  - `pub(crate) fn sanitize_path(path: &Path) -> PathBuf`
- Produces (in `format.rs`):
  - `pub enum Format { Zip, Tar, TarGz, TarBz2, TarXz, TarZst, Gz, Bz2, Xz, Zst, SevenZ, Rar }` (derive `Copy, Clone, Debug, PartialEq, Eq`)
  - `pub fn detect_for_read(path: &Path) -> Result<Format>`
  - `pub fn detect_for_write(path: &Path) -> Result<Format>`

- [ ] **Step 1: Write detection tests (`tests/detection.rs`)**

```rust
use rust_zip_archive::archive::format::{detect_for_read, detect_for_write, Format};
use std::io::Write;

fn write_bytes(path: &std::path::Path, bytes: &[u8]) {
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(bytes).unwrap();
}

#[test]
fn detect_read_by_magic() {
    let dir = tempfile::tempdir().unwrap();
    let cases: &[(&str, &[u8], Format)] = &[
        ("a.zip", &[0x50, 0x4B, 0x03, 0x04], Format::Zip),
        ("a.gz", &[0x1F, 0x8B, 0x08, 0x00], Format::Gz),
        ("a.xz", &[0xFD, b'7', b'z', b'X', b'Z', 0x00], Format::Xz),
        ("a.zst", &[0x28, 0xB5, 0x2F, 0xFD], Format::Zst),
        ("a.bz2", &[0x42, 0x5A, 0x68, 0x39], Format::Bz2),
    ];
    for (name, magic, want) in cases {
        let p = dir.path().join(name);
        write_bytes(&p, magic);
        assert_eq!(detect_for_read(&p).unwrap(), *want, "{name}");
    }
}

#[test]
fn detect_write_by_extension() {
    assert_eq!(detect_for_write(std::path::Path::new("x.zip")).unwrap(), Format::Zip);
    assert_eq!(detect_for_write(std::path::Path::new("x.tar")).unwrap(), Format::Tar);
    assert_eq!(detect_for_write(std::path::Path::new("x.tar.gz")).unwrap(), Format::TarGz);
    assert_eq!(detect_for_write(std::path::Path::new("x.tgz")).unwrap(), Format::TarGz);
    assert_eq!(detect_for_write(std::path::Path::new("x.tar.zst")).unwrap(), Format::TarZst);
    assert_eq!(detect_for_write(std::path::Path::new("x.gz")).unwrap(), Format::Gz);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --test detection`
Expected: FAIL to compile — `archive::format` module doesn't exist.

- [ ] **Step 3: Create `src/archive/format.rs`**

```rust
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
        return Ok(if ext_matches(path, ".tar.gz") || ext_matches(path, ".tgz") {
            Format::TarGz
        } else {
            Format::Gz
        });
    }
    if starts(&[0xFD, b'7', b'z', b'X', b'Z', 0x00]) {
        return Ok(if ext_matches(path, ".tar.xz") || ext_matches(path, ".txz") {
            Format::TarXz
        } else {
            Format::Xz
        });
    }
    if starts(&[0x28, 0xB5, 0x2F, 0xFD]) {
        return Ok(if ext_matches(path, ".tar.zst") || ext_matches(path, ".tzst") {
            Format::TarZst
        } else {
            Format::Zst
        });
    }
    if starts(&[0x42, 0x5A, 0x68]) {
        return Ok(if ext_matches(path, ".tar.bz2") || ext_matches(path, ".tbz2") {
            Format::TarBz2
        } else {
            Format::Bz2
        });
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
```

- [ ] **Step 4: Create `src/archive/zip.rs` by moving the ZIP logic**

Move the current ZIP-specific functions out of `src/archive.rs` into `src/archive/zip.rs`. Rename the public functions to `pub(crate)` and keep their bodies. The shared helpers (`Entry`, `collect_entries`, `to_archive_name`, `sanitize_path`) do NOT go here — they move to `mod.rs` (Step 5) and `zip.rs` calls them via `use super::{collect_entries, sanitize_path, EntryInfo, Progress};`.

`src/archive/zip.rs` full content:

```rust
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
        bail!("{} already exists (use --force to overwrite)", output.display());
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
        progress(Progress { current: i as u64, total, message: entry.name.clone() });
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
    }
    zip.finish().context("finalizing archive")?;
    progress(Progress { current: total, total, message: "done".into() });
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
        progress(Progress { current: i as u64, total, message: safe_name.display().to_string() });
        if entry.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            if outpath.exists() && !force {
                bail!("{} already exists (use --force to overwrite)", outpath.display());
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
    progress(Progress { current: total, total, message: "done".into() });
    Ok(())
}
```

- [ ] **Step 5: Create `src/archive/mod.rs` with shared helpers + dispatch**

```rust
//! Multi-format archive API. Detects the format and dispatches to a backend.

pub mod format;
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
    match format::detect_for_write(output)? {
        Format::Zip => zip::create(output, inputs, compression, force, progress),
        other => bail!("creating {:?} archives is not supported yet", other),
    }
}

pub fn list(archive: &Path) -> Result<Vec<EntryInfo>> {
    match format::detect_for_read(archive)? {
        Format::Zip => zip::list(archive),
        other => bail!("listing {:?} archives is not supported yet", other),
    }
}

pub fn extract(
    archive: &Path,
    dest: &Path,
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    match format::detect_for_read(archive)? {
        Format::Zip => zip::extract(archive, dest, force, progress),
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
    match format::detect_for_read(archive)? {
        Format::Zip => zip::extract_selected(archive, dest, names, force, progress),
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
```

Then delete `src/archive.rs`.

- [ ] **Step 6: Run all tests + lint**

Run: `source "$HOME/.cargo/env" && cargo test && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings`
Expected: `tests/roundtrip.rs` (5) and `tests/detection.rs` (2) pass; fmt + clippy clean.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: archive module tree + format detection/dispatch"
```

---

### Task 2: tar + tarball backend

Add `.tar`, `.tar.gz`, `.tar.bz2`, `.tar.xz`, `.tar.zst` create/list/extract/extract_selected, wired into the dispatcher.

**Files:**
- Create: `src/archive/tar.rs`
- Modify: `src/archive/mod.rs` (add `mod tar;` and dispatch arms)
- Modify: `Cargo.toml` (add `tar`, `flate2`, `bzip2`, `xz2`, `zstd` deps)
- Test: `tests/formats_tar.rs`

**Interfaces:**
- Consumes: `super::{collect_entries, sanitize_path, EntryInfo, Progress}`, `format::Format`.
- Produces (in `tar.rs`, all `pub(crate)`):
  - `fn create(output: &Path, inputs: &[PathBuf], format: Format, force: bool, progress: impl FnMut(Progress)) -> Result<()>`
  - `fn list(archive: &Path, format: Format) -> Result<Vec<EntryInfo>>`
  - `fn extract(archive: &Path, dest: &Path, format: Format, force: bool, progress: impl FnMut(Progress)) -> Result<()>`
  - `fn extract_selected(archive: &Path, dest: &Path, names: &[String], format: Format, force: bool, progress: impl FnMut(Progress)) -> Result<()>`

- [ ] **Step 1: Add dependencies to `Cargo.toml`**

Under `[dependencies]` add:

```toml
tar = "0.4"
flate2 = "1"
bzip2 = "0.5"
xz2 = "0.1"
zstd = "0.13"
```

- [ ] **Step 2: Write the round-trip tests (`tests/formats_tar.rs`)**

```rust
use rust_zip_archive::archive;
use rust_zip_archive::cli::Compression;
use std::fs;
use std::path::Path;

fn write(path: &Path, contents: &str) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn round_trip(ext: &str) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = root.join("sample");
    write(&src.join("a.txt"), &"hello\n".repeat(50));
    write(&src.join("nested/b.txt"), "deep\n");

    let archive = root.join(format!("out{ext}"));
    archive::create(&archive, &[src.clone()], Compression::Deflate, false, |_p| {}).unwrap();
    assert!(archive.exists(), "{ext}: not created");

    let entries = archive::list(&archive).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.iter().any(|n| n.ends_with("a.txt")), "{ext}: a.txt missing in list");

    let dest = root.join("out");
    archive::extract(&archive, &dest, false, |_p| {}).unwrap();
    let restored = fs::read_to_string(dest.join("sample/a.txt")).unwrap();
    assert_eq!(restored, "hello\n".repeat(50), "{ext}: content mismatch");
}

#[test] fn tar_round_trip() { round_trip(".tar"); }
#[test] fn tar_gz_round_trip() { round_trip(".tar.gz"); }
#[test] fn tar_bz2_round_trip() { round_trip(".tar.bz2"); }
#[test] fn tar_xz_round_trip() { round_trip(".tar.xz"); }
#[test] fn tar_zst_round_trip() { round_trip(".tar.zst"); }

#[test]
fn tar_extract_selected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = root.join("s");
    write(&src.join("keep.txt"), "k");
    write(&src.join("skip.txt"), "s");
    let archive = root.join("a.tar");
    archive::create(&archive, &[src.clone()], Compression::Deflate, false, |_p| {}).unwrap();
    let dest = root.join("out");
    archive::extract_selected(&archive, &dest, &["s/keep.txt".into()], false, |_p| {}).unwrap();
    assert!(dest.join("s/keep.txt").exists());
    assert!(!dest.join("s/skip.txt").exists());
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --test formats_tar`
Expected: FAIL — create returns "not supported yet" / panics on unwrap.

- [ ] **Step 4: Create `src/archive/tar.rs`**

```rust
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
        bail!("{} already exists (use --force to overwrite)", output.display());
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
            let enc = zstd::stream::write::Encoder::new(w, 0)?.auto_finish();
            let mut b = tar::Builder::new(enc);
            write_entries(&mut b, &entries, progress)?;
            b.into_inner()?; // auto_finish writer finalizes on drop
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
        progress(Progress { current: i as u64, total, message: entry.name.clone() });
        // append_path_with_name reads metadata (mode, mtime) and contents from
        // disk and stores them under the archive-relative name.
        builder
            .append_path_with_name(&entry.path, &entry.name)
            .with_context(|| format!("adding {}", entry.name))?;
    }
    progress(Progress { current: total, total, message: "done".into() });
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

pub(crate) fn list(archive: &Path, format: Format) -> Result<Vec<EntryInfo>> {
    with_reader(archive, format, |mut ar| {
        let mut out = Vec::new();
        for entry in ar.entries()? {
            let entry = entry?;
            let header = entry.header();
            let is_dir = header.entry_type().is_dir();
            let size = header.size().unwrap_or(0);
            let name = entry.path()?.to_string_lossy().to_string();
            out.push(EntryInfo { name, size, compressed: size, is_dir });
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
            if raw.is_absolute() || raw.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                bail!("unsafe path in archive: {}", raw_str);
            }
            let safe = sanitize_path(&raw);
            let outpath = dest.join(&safe);
            progress(Progress { current: idx, total: 0, message: safe.display().to_string() });
            idx += 1;

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&outpath)?;
                continue;
            }
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            if outpath.exists() && !force {
                bail!("{} already exists (use --force to overwrite)", outpath.display());
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
        progress(Progress { current: idx, total: idx, message: "done".into() });
        Ok(())
    })
}
```

- [ ] **Step 5: Wire tar into `src/archive/mod.rs`**

Add `mod tar;` near the top (under `mod zip;`). Replace the four dispatchers' bodies to route tar formats. For `create`:

```rust
pub fn create(
    output: &Path,
    inputs: &[PathBuf],
    compression: Compression,
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    match format::detect_for_write(output)? {
        Format::Zip => zip::create(output, inputs, compression, force, progress),
        f @ (Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst) => {
            tar::create(output, inputs, f, force, progress)
        }
        other => bail!("creating {:?} archives is not supported yet", other),
    }
}
```

For `list`:

```rust
pub fn list(archive: &Path) -> Result<Vec<EntryInfo>> {
    match format::detect_for_read(archive)? {
        Format::Zip => zip::list(archive),
        f @ (Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst) => {
            tar::list(archive, f)
        }
        other => bail!("listing {:?} archives is not supported yet", other),
    }
}
```

For `extract`:

```rust
pub fn extract(
    archive: &Path,
    dest: &Path,
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    match format::detect_for_read(archive)? {
        Format::Zip => zip::extract(archive, dest, force, progress),
        f @ (Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst) => {
            tar::extract(archive, dest, f, force, progress)
        }
        other => bail!("extracting {:?} archives is not supported yet", other),
    }
}
```

For `extract_selected`:

```rust
pub fn extract_selected(
    archive: &Path,
    dest: &Path,
    names: &[String],
    force: bool,
    progress: impl FnMut(Progress),
) -> Result<()> {
    match format::detect_for_read(archive)? {
        Format::Zip => zip::extract_selected(archive, dest, names, force, progress),
        f @ (Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst) => {
            tar::extract_selected(archive, dest, names, f, force, progress)
        }
        other => bail!("extracting {:?} archives is not supported yet", other),
    }
}
```

- [ ] **Step 6: Run tests + lint**

Run: `source "$HOME/.cargo/env" && cargo test && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings`
Expected: `tests/formats_tar.rs` (6) pass plus all prior tests; fmt + clippy clean.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: tar and tarball (gz/bz2/xz/zst) support"
```

---

### Task 3: single-file compressor backend

Add standalone `.gz`, `.bz2`, `.xz`, `.zst` (compress exactly one file; list shows one entry; extract writes the decompressed file).

**Files:**
- Create: `src/archive/compressor.rs`
- Modify: `src/archive/mod.rs` (add `mod compressor;` + dispatch arms + capability error for multi-input)
- Test: `tests/formats_compressor.rs`

**Interfaces:**
- Consumes: `super::{EntryInfo, Progress}`, `format::Format`.
- Produces (in `compressor.rs`, `pub(crate)`):
  - `fn create(output: &Path, input: &Path, format: Format, force: bool, progress: impl FnMut(Progress)) -> Result<()>`
  - `fn list(archive: &Path, format: Format) -> Result<Vec<EntryInfo>>`
  - `fn extract(archive: &Path, dest: &Path, format: Format, force: bool, progress: impl FnMut(Progress)) -> Result<()>`
  - `fn inner_name(archive: &Path, format: Format) -> String` (strips the compressor extension)

- [ ] **Step 1: Write tests (`tests/formats_compressor.rs`)**

```rust
use rust_zip_archive::archive;
use rust_zip_archive::cli::Compression;
use std::fs;
use std::path::Path;

fn write(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

fn round_trip(ext: &str) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let input = root.join("data.txt");
    let body = "compress me\n".repeat(100);
    write(&input, &body);

    let archive = root.join(format!("data.txt{ext}"));
    archive::create(&archive, &[input.clone()], Compression::Deflate, false, |_p| {}).unwrap();
    assert!(archive.exists(), "{ext}: not created");

    let entries = archive::list(&archive).unwrap();
    assert_eq!(entries.len(), 1, "{ext}: should list exactly one entry");
    assert_eq!(entries[0].name, "data.txt", "{ext}: inner name");

    let dest = root.join("out");
    archive::extract(&archive, &dest, false, |_p| {}).unwrap();
    let restored = fs::read_to_string(dest.join("data.txt")).unwrap();
    assert_eq!(restored, body, "{ext}: content mismatch");
}

#[test] fn gz_round_trip() { round_trip(".gz"); }
#[test] fn bz2_round_trip() { round_trip(".bz2"); }
#[test] fn xz_round_trip() { round_trip(".xz"); }
#[test] fn zst_round_trip() { round_trip(".zst"); }

#[test]
fn gz_rejects_multiple_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let a = root.join("a.txt"); write(&a, "a");
    let b = root.join("b.txt"); write(&b, "b");
    let archive = root.join("out.gz");
    let err = archive::create(&archive, &[a, b], Compression::Deflate, false, |_p| {}).unwrap_err();
    assert!(err.to_string().contains("single file"), "expected single-file error, got: {err}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --test formats_compressor`
Expected: FAIL — create returns "not supported yet".

- [ ] **Step 3: Create `src/archive/compressor.rs`**

```rust
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
        bail!("{} already exists (use --force to overwrite)", output.display());
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut reader = BufReader::new(
        File::open(input).with_context(|| format!("reading {}", input.display()))?,
    );
    let w = BufWriter::new(
        File::create(output).with_context(|| format!("creating {}", output.display()))?,
    );
    progress(Progress { current: 0, total: 1, message: input.display().to_string() });
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
    progress(Progress { current: 1, total: 1, message: "done".into() });
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
        bail!("{} already exists (use --force to overwrite)", outpath.display());
    }
    progress(Progress { current: 0, total: 1, message: outpath.display().to_string() });
    let mut dec = open_decoder(archive, format)?;
    let mut out = BufWriter::new(File::create(&outpath)?);
    io::copy(&mut dec, &mut out)?;
    out.flush()?;
    progress(Progress { current: 1, total: 1, message: "done".into() });
    Ok(())
}
```

- [ ] **Step 4: Wire compressor into `src/archive/mod.rs`**

Add `mod compressor;` near the top. In `create`, single-file compressors need the single-input guard. Update the `create` dispatcher:

```rust
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
```

Update `list`, `extract`, `extract_selected` to add compressor arms:

```rust
// in list():
        Format::Gz | Format::Bz2 | Format::Xz | Format::Zst => {
            compressor::list(archive, format::detect_for_read(archive)?)
        }
```

(Compute the format once at the top of each function to avoid double-detection. Refactor each of `list`/`extract`/`extract_selected` to `let format = format::detect_for_read(archive)?;` then `match format`.) For `extract`:

```rust
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
```

For `extract_selected`, single-file compressors ignore the selection (one file): the compressor arm calls `compressor::extract(archive, dest, format, force, progress)`. Apply the same `let format = ...; match` shape to `list` and `extract_selected`.

- [ ] **Step 5: Run tests + lint**

Run: `source "$HOME/.cargo/env" && cargo test && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings`
Expected: `tests/formats_compressor.rs` (5) pass plus all prior; fmt + clippy clean.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: standalone gz/bz2/xz/zst single-file compression"
```

---

### Task 4: CLI help, GUI dialog filters, and docs

Surface the new formats to users: CLI help text, GUI file-dialog filters, and README.

**Files:**
- Modify: `src/cli.rs` (help text noting multi-format + `--method` is zip-only)
- Modify: `src/bin/rza-gui.rs` (expand rfd filters)
- Modify: `README.md`
- Test: `tests/cli_formats.rs` (smoke via the binary)

**Interfaces:**
- Consumes: existing CLI/GUI; library multi-format API.

- [ ] **Step 1: Write a CLI smoke test (`tests/cli_formats.rs`)**

```rust
use std::process::Command;

fn rza() -> Command { Command::new(env!("CARGO_BIN_EXE_rza")) }

#[test]
fn cli_creates_and_lists_tar_gz() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let f = root.join("hello.txt");
    std::fs::write(&f, "hi\n").unwrap();
    let archive = root.join("out.tar.gz");

    assert!(rza().arg("create").arg("-o").arg(&archive).arg(&f).status().unwrap().success());
    assert!(archive.exists());

    let out = rza().arg("list").arg(&archive).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("hello.txt"));
}
```

- [ ] **Step 2: Run test to verify it fails or passes**

Run: `source "$HOME/.cargo/env" && cargo test --test cli_formats`
Expected: PASS already (library + binary support tar.gz from Task 2). If it fails, fix the wiring before continuing. This test guards the end-to-end CLI path.

- [ ] **Step 3: Update CLI help text in `src/cli.rs`**

Change the top-level `about` and the `Create` command doc. Update the doc comment on `Create`'s `output` arg and `method` arg:

```rust
        /// Path of the archive to create. The format is chosen by the
        /// extension: .zip, .tar, .tar.gz/.tgz, .tar.bz2, .tar.xz, .tar.zst,
        /// or single-file .gz/.bz2/.xz/.zst.
        #[arg(short, long)]
        output: PathBuf,
```

```rust
        /// Compression method (applies to .zip only; other formats use the
        /// compression implied by their extension).
        #[arg(short, long, value_enum, default_value_t = Compression::Deflate)]
        method: Compression,
```

And update the struct-level doc comment:

```rust
/// rza — a small multi-format archive utility (zip, tar, tar.gz/bz2/xz/zst, gz/bz2/xz/zst).
#[derive(Parser, Debug)]
#[command(name = "rza", version, about, long_about = None)]
pub struct Cli {
```

- [ ] **Step 4: Expand GUI file-dialog filters in `src/bin/rza-gui.rs`**

Find the "Open Archive…" dialog call and replace its filter so all readable formats are offered:

```rust
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(
                                "Archives",
                                &["zip", "tar", "gz", "tgz", "bz2", "xz", "zst", "7z", "rar"],
                            )
                            .pick_file()
                        {
                            self.open_archive(path);
                        }
```

And the create "Create…" save dialog filter (in `start_create`):

```rust
        let Some(output) = rfd::FileDialog::new()
            .add_filter("Archives", &["zip", "tar", "gz", "tgz", "bz2", "xz", "zst"])
            .set_file_name("archive.zip")
            .save_file()
        else {
            return;
        };
```

- [ ] **Step 5: Update `README.md`**

Replace the Features "Create/Extract/List .zip" bullets to reflect multi-format, and add a Formats table. Add after the existing feature list:

```markdown
## Supported formats

| Format | List | Extract | Create |
|--------|------|---------|--------|
| `.zip` | ✅ | ✅ | ✅ |
| `.tar`, `.tar.gz`/`.tgz`, `.tar.bz2`, `.tar.xz`, `.tar.zst` | ✅ | ✅ | ✅ |
| `.gz`, `.bz2`, `.xz`, `.zst` (single file) | ✅ | ✅ | ✅ |

The format is auto-detected on extract/list (by content), and chosen from the
output extension on create. `--method` applies to `.zip` only.
```

Also remove the now-done `.tar`/`.tar.gz`/`.tar.zst` and `xz` lines from "Roadmap ideas".

- [ ] **Step 6: Run full suite + lint**

Run: `source "$HOME/.cargo/env" && cargo test && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo clippy --features gui --bin rza-gui -- -D warnings`
Expected: all tests pass; fmt + clippy (default and gui) clean.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: surface multi-format support in CLI help, GUI dialogs, and docs"
```

---

## Self-Review Notes

- **Spec coverage (Plan A portion):** module tree + dispatch (Task 1); format detection magic/extension (Task 1, `format.rs`); tar + tarballs create/list/extract/extract_selected (Task 2); single-file compressors + single-input capability error (Task 3); CLI help + GUI filters + README (Task 4). Zip-slip guard preserved (zip.rs `enclosed_name`; tar.rs explicit absolute/`..` rejection) and Windows sanitize via shared `sanitize_path`. 7z/rar are intentionally deferred to Plans B and C (dispatch returns "not supported yet" for them, and detection already recognizes their magic).
- **Type consistency:** `Progress`/`EntryInfo` defined in `mod.rs` Task 1, reused unchanged by zip/tar/compressor; `Format` defined in `format.rs` Task 1, consumed by tar (Task 2) and compressor (Task 3); shared `collect_entries`/`to_archive_name`/`sanitize_path` defined in `mod.rs` Task 1, reused by zip (Task 1) and tar (Task 2).
- **Crate-API caveat:** tar/flate2/bzip2/xz2/zstd APIs used here match their documented 0.4/1/0.5/0.1/0.13 surfaces; if a minor name differs in the installed version (e.g. `zstd::stream::write::Encoder::new`/`auto_finish`), the implementer adapts to the crate's docs and notes it in the task report.
- **Known limitation (documented):** tar extraction applies absolute/`..` rejection + Windows `sanitize_path`; symlink entries are written via the file path (tar symlink fidelity is out of scope for this slice).
