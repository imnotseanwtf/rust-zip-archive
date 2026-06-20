# Richer Right-Click Menu (Shell Menu v2) — Design Spec

**Date:** 2026-06-20
**Status:** Approved

## Goal

Expand the Windows right-click **rza ▸** menu toward 7-Zip's breadth using the
static-registry approach (chosen over the COM-DLL path): add **Test archive**,
**Open with rza**, and quick-compress **Compress to .zip** / **Compress to
.tar.gz**, and make the menu work on **folders** too. Backed by new CLI
subcommands.

Explicitly NOT in this slice (chosen tier): dynamic "{filename}" labels and the
cascading-icon look (those need a COM shell-extension DLL); `.7z` entries (need
Plan B's 7z writing); compress-and-email; CRC submenu; multi-select.

## Why static labels

Windows static-registry context entries can have many actions but **fixed label
text**. So menu items read "Compress to .zip" / "Extract to subfolder", not
"Add to photos.zip". The invoked command still computes the correct output name;
only the displayed label is generic. Dynamic per-file labels require a COM DLL,
which cannot be built or tested in this project's environment and was declined.

## 1. New CLI subcommands

Added to `src/cli.rs` (clap `Command` variants) and handled in `src/bin/rza.rs`,
reusing the library:

- `rza test <archive>` — run `archive::test(archive, progress)`; on success print
  a confirmation, on failure return the error (non-zero exit). Uses the same
  progress bar style as other commands.
- `rza compress-zip <path>` — create `<archive_output(path, ".zip")>` from
  `path`, using `Compression::Deflate`.
- `rza compress-targz <path>` — create `<archive_output(path, ".tar.gz")>` from
  `path` (format inferred from the `.tar.gz` extension by the existing
  `create` dispatch).

Both compress commands pass `force = false` (don't overwrite an existing
archive; surface the existing-file error).

## 2. Output-name helper (pure, unit-tested)

```rust
/// Output archive path for a quick-compress action:
/// parent directory + base name + ext, where base name is the file stem for a
/// file (e.g. report.docx -> report) or the directory name for a folder
/// (e.g. photos/ -> photos). `ext` includes the leading dot (".zip", ".tar.gz").
fn archive_output(path: &Path, ext: &str) -> PathBuf;
```

Rules:
- Folder `photos` → `photos.zip` (uses the directory name).
- File `report.docx` → `report.zip` (strips the last extension only).
- File with no extension `notes` → `notes.zip`.
- Parent is the item's own parent (archive lands next to the item); empty parent
  falls back to ".".
- `ext` is appended verbatim, so `.tar.gz` yields `photos.tar.gz`.

## 3. Menu structure (registry)

Cascading **rza** submenus written per-user (`HKCU\Software\Classes`):

- **Archive types** (`.zip .tar .gz .tgz .bz2 .xz .zst .7z .rar`), under
  `SystemFileAssociations\<ext>\shell\rza` (extends the existing submenu):
  - Extract Here → `rza.exe extract-here "%1"`
  - Extract to subfolder → `rza.exe extract-to "%1"`
  - **Test archive** → `rza.exe test "%1"`
  - Open with rza → `rza-gui.exe "%1"`
- **All files** (`*\shell\rza`) and **folders** (`Directory\shell\rza`) — a
  cascading submenu replacing the current single `*\shell\rza.add` entry:
  - Add to archive… → `rza-gui.exe "%1"`
  - **Compress to .zip** → `rza.exe compress-zip "%1"`
  - **Compress to .tar.gz** → `rza.exe compress-targz "%1"`

All commands quote `"$INSTDIR\rza.exe"` / `"$INSTDIR\rza-gui.exe"` and `"%1"`.

The registry block lives in `packaging/windows/shell-menu.nsh` (canonical) and is
mirrored in `Cargo.toml`'s `preinstall-section` (cargo-packager can't `!include`
an external file); both copies are kept byte-consistent (modulo TOML `\\`
escaping). The accepted v1 limitation stands: uninstall does not remove these
per-user keys (no cargo-packager uninstall hook).

## 4. Error handling

- `rza test` failure → non-zero exit + the error message (shell action surfaces a
  real error window rather than silently doing nothing).
- `compress-*` when the output already exists → the existing-file error from
  `create` (force=false), non-zero exit.
- `archive_output` with an empty/`.`-only parent → archive written in the current
  directory.

## 5. Testing

- **Unit** (Linux): `archive_output` — folder name, file-stem stripping,
  no-extension, `.tar.gz` two-part ext, empty-parent fallback.
- **Integration** (Linux, via the built `rza` binary): `rza compress-zip <dir>`
  produces `<dir>.zip` containing the folder; `rza compress-targz <dir>` produces
  `<dir>.tar.gz`; `rza test <good>` exits 0; `rza test <corrupt>` exits non-zero.
- **Registry menu** (Windows, MANUAL by user): build the installer in CI, install,
  verify the expanded submenus and each action; iterate on the NSIS block as
  needed. Not verifiable in this environment.

## 6. Scope guard (YAGNI)

**In:** `rza test` / `compress-zip` / `compress-targz` CLI + `archive_output`
helper; expanded archive submenu (add Test); new `*` and `Directory` cascading
submenus (Add / Compress to zip / Compress to tar.gz); README update.
**Out:** COM DLL / dynamic labels; `.7z` menu entries (Plan B); compress-and-email;
CRC/hash submenu; multi-select handling; uninstall key removal.
