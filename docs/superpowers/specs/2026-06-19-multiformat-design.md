# Multi-Format Support — Design Spec

**Date:** 2026-06-19
**Status:** Approved (pending spec review)

## Goal

Extend `rza` beyond ZIP to read/write many archive and compression formats,
moving it toward a 7-Zip-style general archiver. This is the first slice of a
larger "7-Zip/WinRAR features" effort (later slices: encryption, archive
editing, split volumes, SFX, shell integration).

## Formats in this release

| Format | List | Extract | Create | Notes |
|--------|------|---------|--------|-------|
| `.zip` | ✅ | ✅ | ✅ | existing |
| `.tar` | ✅ | ✅ | ✅ | uncompressed container |
| `.tar.gz` / `.tgz` | ✅ | ✅ | ✅ | tar + gzip |
| `.tar.bz2` | ✅ | ✅ | ✅ | tar + bzip2 |
| `.tar.xz` | ✅ | ✅ | ✅ | tar + xz |
| `.tar.zst` | ✅ | ✅ | ✅ | tar + zstd |
| `.gz` | ✅ | ✅ | ✅ | single file only |
| `.bz2` | ✅ | ✅ | ✅ | single file only |
| `.xz` | ✅ | ✅ | ✅ | single file only |
| `.zst` | ✅ | ✅ | ✅ | single file only |
| `.7z` | ✅ | ✅ | ✅ | feature `sevenz` (default on) |
| `.rar` | ✅ | ✅ | ❌ | feature `rar` (default on); create impossible (proprietary) |

## 1. Architecture: format-abstraction layer

`src/archive.rs` (ZIP-only today) becomes a module tree:

```
src/archive/
  mod.rs         → public API (create/list/extract/extract_selected); detects Format and dispatches; re-exports Progress/EntryInfo
  format.rs      → Format enum, detection (magic bytes + extension fallback), capability queries
  zip.rs         → existing ZIP logic, moved verbatim
  tar.rs         → tar + tarball combos (streaming compressor layer over tar)
  compressor.rs  → single-file gz/bz2/xz/zst (compress one file / decompress one file)
  sevenz.rs      → .7z read+write          (#[cfg(feature = "sevenz")])
  rar.rs         → .rar list+extract        (#[cfg(feature = "rar")])
```

Dispatch is a `match Format { … }` in `mod.rs`, **not** trait objects: the
shared progress callback is a generic `impl FnMut(Progress)` which is not
object-safe. Each backend module is an isolated, independently testable unit
exposing plain functions.

## 2. Format taxonomy

Three behavioral categories:

1. **Multi-entry archives** (many files/dirs): zip, tar, 7z, rar.
2. **Single-file compressors** (exactly one file): gz, bz2, xz, zst.
3. **Tarball combos**: tar piped through a compressor.

## 3. Format detection (`format.rs`)

```rust
pub enum Format {
    Zip, Tar, TarGz, TarBz2, TarXz, TarZst,
    Gz, Bz2, Xz, Zst,
    SevenZ, Rar,
}

/// Detect the format of an existing archive for list/extract.
/// Reads the first bytes for a magic signature; falls back to extension.
pub fn detect_for_read(path: &Path) -> Result<Format>;

/// Choose the output format for create() from the path's extension.
pub fn detect_for_write(path: &Path) -> Result<Format>;

/// True if this format supports create().
pub fn supports_create(f: Format) -> bool;  // false only for Rar
```

**Magic signatures:** zip `50 4B 03 04`; gzip `1F 8B`; xz `FD 37 7A 58 5A 00`;
zstd `28 B5 2F FD`; bzip2 `42 5A 68` (`BZh`); 7z `37 7A BC AF 27 1C`;
rar `52 61 72 21` (`Rar!`).

**Tarball vs single file (gz/bz2/xz/zst):** decided by extension —
`.tar.gz`/`.tgz` → tar inside; bare `.gz` → single file. When a bare
`.gz`/`.xz`/`.bz2`/`.zst` is opened for list/extract and the extension is
ambiguous, peek for the `ustar` magic at offset 257 of the decompressed first
block; if present, treat as a tarball.

## 4. Public API (signatures unchanged; behavior generalized)

`mod.rs` keeps the existing signatures and dispatches:

```rust
pub fn create(output: &Path, inputs: &[PathBuf], compression: Compression, force: bool, progress: impl FnMut(Progress)) -> Result<()>;
pub fn list(archive: &Path) -> Result<Vec<EntryInfo>>;
pub fn extract(archive: &Path, dest: &Path, force: bool, progress: impl FnMut(Progress)) -> Result<()>;
pub fn extract_selected(archive: &Path, dest: &Path, names: &[String], force: bool, progress: impl FnMut(Progress)) -> Result<()>;
```

`Progress` and `EntryInfo` are unchanged and shared across all backends, so the
CLI and GUI work with every format without changes to their call sites.

### Capability rules (clear errors)

- `create` to `.rar` → `bail!("creating .rar archives is not supported (proprietary format); rza can only extract .rar")`.
- `create` to a single-file compressor (`.gz/.bz2/.xz/.zst`) with more than one
  input, or a directory input → `bail!("<ext> compresses a single file; use .tar.<ext> to archive multiple files or a directory")`.
- `--method` (zip's `Store`/`Deflate`/`Bzip2`/`Zstd`) applies to `.zip` only;
  other formats derive compression from their extension. Documented in CLI help.

### Single-file compressor semantics

- `create file.txt.gz` from `file.txt` → gzip-compress the one file.
- `list file.txt.gz` → one `EntryInfo` (the inner filename, derived by stripping
  the compressor extension; size = decompressed size if cheaply known, else 0).
- `extract file.txt.gz` → write `file.txt` into the destination.

## 5. Dependencies & feature flags

`Cargo.toml`:

```toml
[dependencies]
tar = "0.4"
flate2 = "1"
# bzip2, xz2, zstd already present transitively via zip; add direct deps:
bzip2 = "0.5"
xz2 = "0.1"
zstd = "0.13"
sevenz-rust = { version = "0.6", optional = true }
unrar = { version = "0.5", optional = true }

[features]
default = ["sevenz", "rar"]
sevenz = ["dep:sevenz-rust"]
rar = ["dep:unrar"]
gui = ["dep:eframe", "dep:egui", "dep:rfd"]
```

- Core formats build cross-platform today (bzip2/xz/zstd C sources are bundled).
- `sevenz` (pure Rust) default-on, low risk.
- `rar` default-on; `unrar` bundles C++ source (no system lib needed) but adds
  per-OS build risk. Keeping it a feature lets us disable just rar on a
  problematic target via `--no-default-features --features sevenz,...` without
  losing other formats.

## 6. CLI & GUI impact

- **CLI:** `create` infers format from `-o`'s extension; `list`/`extract`
  auto-detect via `detect_for_read`. `--method` documented as zip-only.
- **GUI:** gains multi-format **list/extract automatically** (open `.7z`,
  `.rar`, `.tar.gz`, …). For **create**, the format is chosen by the extension
  the user types in the Save dialog; the Method dropdown stays zip-specific.
  Open/Save dialog filters expand to include the new extensions.

## 7. Error handling

- Unknown/unsupported signature → `bail!("unrecognized or unsupported archive format: <path>")`.
- Corrupt streams surface the backend's error via `anyhow` context.
- Existing zip-slip protection (`enclosed_name`) and Windows name sanitization
  apply to every multi-entry backend's extraction path; tar extraction must
  apply the same path-safety checks (reject `..`/absolute, sanitize on Windows).

## 8. Testing

- One integration test module per format. For creatable formats:
  create → list → extract → `diff` round-trip (assert content identical;
  on Unix assert executable bit where the format preserves it: tar/zip/7z).
- `.rar`: commit a tiny fixture `tests/fixtures/sample.rar` (a known 2-file
  archive); test `list` and `extract` against it.
- Single-file compressors: compress a file → decompress → assert bytes equal.
- Detection unit tests: feed known magic-byte prefixes to `detect_for_read`.
- CI: add `sevenz` and `rar` feature builds/tests on all three OSes.

## 9. Scope guard (YAGNI)

**In:** list/extract for all listed formats; create for zip/tar/tarballs/7z/
single-compressors. **Out (future slices):** encryption/passwords, editing
archives in place, split/multi-volume, self-extracting archives, nested-archive
browsing, compression-level/threading tuning.
