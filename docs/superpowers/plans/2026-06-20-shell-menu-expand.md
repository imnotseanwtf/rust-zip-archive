# Richer Right-Click Menu (Shell Menu v2) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `rza test` / `compress-zip` / `compress-targz` CLI commands and expand the Windows right-click **rza ▸** menu (Test archive; Compress to .zip / .tar.gz on files & folders) using static registry entries.

**Architecture:** New clap `Command` variants + handlers in the CLI binary, reusing `archive::{create,test}`, with a pure `archive_output` naming helper. The registry menu is extended in `packaging/windows/shell-menu.nsh` and the mirrored `Cargo.toml` `preinstall-section`. CLI is fully testable on Linux; the registry menu is verified on Windows by the user.

**Tech Stack:** Rust 2021; existing archive library + clap CLI; cargo-packager / NSIS for the installer registry keys.

## Global Constraints

- Edition 2021; `cargo build` (no features) stays CLI-only; no new runtime deps.
- Menu labels are **fixed text** (e.g. "Compress to .zip"), not dynamic "{name}.zip"; the command computes the correct output name.
- Registry keys are per-user (`HKCU\Software\Classes`); the v1 limitation that uninstall does not remove them stands.
- `packaging/windows/shell-menu.nsh` is canonical; `Cargo.toml`'s `preinstall-section` mirrors it byte-for-byte (modulo TOML `\\` escaping).
- `compress-*` commands use `force=false` and act on the single given path.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` clean.

---

### Task 1: CLI `test`, `compress-zip`, `compress-targz` + `archive_output`

Add the three shell-callable subcommands and the pure output-name helper.

**Files:**
- Modify: `src/cli.rs` (three new `Command` variants)
- Modify: `src/bin/rza.rs` (helper + handlers + inline unit tests)
- Test: `tests/shell_compress.rs`

**Interfaces:**
- Produces (in `src/bin/rza.rs`): `fn archive_output(path: &Path, ext: &str) -> PathBuf`.
- Produces (in `src/cli.rs`): `Command::Test { archive: PathBuf }`, `Command::CompressZip { path: PathBuf }`, `Command::CompressTargz { path: PathBuf }`.
- Consumes: `rust_zip_archive::archive::{test, create, Progress}`, `rust_zip_archive::cli::Compression`.

- [ ] **Step 1: Write the `archive_output` unit tests at the bottom of `src/bin/rza.rs`**

Add to the existing `#[cfg(test)] mod tests` (it already tests `extract_here_dir`/`extract_to_dir`):

```rust
    use super::archive_output;

    #[test]
    fn output_strips_file_extension() {
        assert_eq!(archive_output(Path::new("/x/report.docx"), ".zip"), PathBuf::from("/x/report.zip"));
    }

    #[test]
    fn output_no_extension_appends() {
        assert_eq!(archive_output(Path::new("/x/notes"), ".zip"), PathBuf::from("/x/notes.zip"));
    }

    #[test]
    fn output_two_part_ext() {
        assert_eq!(archive_output(Path::new("/x/report.docx"), ".tar.gz"), PathBuf::from("/x/report.tar.gz"));
    }

    #[test]
    fn output_folder_uses_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir(&photos).unwrap();
        assert_eq!(archive_output(&photos, ".zip"), dir.path().join("photos.zip"));
    }

    #[test]
    fn output_empty_parent_uses_dot() {
        assert_eq!(archive_output(Path::new("report.docx"), ".zip"), Path::new(".").join("report.zip"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --bin rza`
Expected: FAIL to compile — `archive_output` not defined.

- [ ] **Step 3: Add `archive_output` to `src/bin/rza.rs`**

Near the other helpers (`extract_here_dir`/`extract_to_dir`):

```rust
/// Output archive path for a quick-compress action: the item's parent dir +
/// base name + `ext`. Base name is the directory name for a folder, or the file
/// stem (last extension removed) for a file. `ext` includes the leading dot.
fn archive_output(path: &Path, ext: &str) -> PathBuf {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let base = if path.is_dir() {
        path.file_name().map(|s| s.to_string_lossy().to_string())
    } else {
        path.file_stem().map(|s| s.to_string_lossy().to_string())
    }
    .unwrap_or_default();
    parent.join(format!("{base}{ext}"))
}
```

- [ ] **Step 4: Add the three `Command` variants in `src/cli.rs`**

Inside `enum Command { ... }` (after the `ExtractTo` variant):

```rust
    /// Test the integrity of an archive (reads every entry to verify it).
    Test {
        /// Archive to test.
        archive: PathBuf,
    },

    /// Compress a file or folder into a .zip next to it (shell quick-compress).
    CompressZip {
        /// File or folder to compress.
        path: PathBuf,
    },

    /// Compress a file or folder into a .tar.gz next to it (shell quick-compress).
    CompressTargz {
        /// File or folder to compress.
        path: PathBuf,
    },
```

- [ ] **Step 5: Add the handlers in `src/bin/rza.rs` `main`**

```rust
        Command::Test { archive } => {
            let bar = make_bar("Testing");
            rust_zip_archive::archive::test(&archive, |p: Progress| {
                bar.set_length(p.total);
                bar.set_position(p.current);
                bar.set_message(p.message);
            })?;
            bar.finish_with_message(format!("OK — {} is valid", archive.display()));
        }

        Command::CompressZip { path } => {
            let output = archive_output(&path, ".zip");
            let bar = make_bar("Archiving");
            rust_zip_archive::archive::create(
                &output,
                std::slice::from_ref(&path),
                rust_zip_archive::cli::Compression::Deflate,
                false,
                |p: Progress| {
                    bar.set_length(p.total);
                    bar.set_position(p.current);
                    bar.set_message(p.message);
                },
            )?;
            bar.finish_with_message(format!("Created {}", output.display()));
        }

        Command::CompressTargz { path } => {
            let output = archive_output(&path, ".tar.gz");
            let bar = make_bar("Archiving");
            rust_zip_archive::archive::create(
                &output,
                std::slice::from_ref(&path),
                rust_zip_archive::cli::Compression::Deflate,
                false,
                |p: Progress| {
                    bar.set_length(p.total);
                    bar.set_position(p.current);
                    bar.set_message(p.message);
                },
            )?;
            bar.finish_with_message(format!("Created {}", output.display()));
        }
```

- [ ] **Step 6: Run unit tests to verify they pass**

Run: `source "$HOME/.cargo/env" && cargo test --bin rza`
Expected: the 5 new `archive_output` tests pass (plus the existing helper tests).

- [ ] **Step 7: Write the integration tests (`tests/shell_compress.rs`)**

```rust
use std::process::Command;

fn rza() -> Command { Command::new(env!("CARGO_BIN_EXE_rza")) }

#[test]
fn compress_zip_creates_named_archive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let folder = root.join("photos");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "a").unwrap();

    assert!(rza().arg("compress-zip").arg(&folder).status().unwrap().success());
    let archive = root.join("photos.zip");
    assert!(archive.exists(), "photos.zip should be created next to the folder");

    let out = rza().arg("list").arg(&archive).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("a.txt"));
}

#[test]
fn compress_targz_creates_named_archive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let folder = root.join("docs");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("b.txt"), "b").unwrap();

    assert!(rza().arg("compress-targz").arg(&folder).status().unwrap().success());
    assert!(root.join("docs.tar.gz").exists(), "docs.tar.gz should be created");
}

#[test]
fn test_command_passes_and_fails() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let f = root.join("data.txt");
    std::fs::write(&f, "hello\n".repeat(50)).unwrap();
    let archive = root.join("good.zip");
    assert!(rza().arg("create").arg("-o").arg(&archive).arg(&f).status().unwrap().success());

    // Good archive → exit 0.
    assert!(rza().arg("test").arg(&archive).status().unwrap().success());

    // Corrupt it (truncate) → non-zero exit.
    let data = std::fs::read(&archive).unwrap();
    std::fs::write(&archive, &data[..data.len() / 2]).unwrap();
    assert!(!rza().arg("test").arg(&archive).status().unwrap().success());
}
```

- [ ] **Step 8: Run all tests + lint**

Run: `source "$HOME/.cargo/env" && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: new tests pass plus all prior; clippy + fmt clean.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(cli): test, compress-zip, compress-targz subcommands"
```

---

### Task 2: Expand the registry menu (Test + Compress on files/folders)

Add **Test archive** to the archive submenu and a cascading **rza ▸** submenu (Add / Compress to .zip / Compress to .tar.gz) for `*` and `Directory`. Edit both the canonical `.nsh` and the mirrored `Cargo.toml` `preinstall-section`.

**Files:**
- Modify: `packaging/windows/shell-menu.nsh`
- Modify: `Cargo.toml` (`[package.metadata.packager]` `preinstall-section`)

**Interfaces:**
- Consumes: the CLI verbs from Task 1 (`test`, `compress-zip`, `compress-targz`) and existing `extract-here`/`extract-to`/`rza-gui`.

- [ ] **Step 1: Read the current `packaging/windows/shell-menu.nsh`**

Run: `cat packaging/windows/shell-menu.nsh` to see the existing per-extension archive blocks and the `*\shell\rza.add` entry, so the edits below match its exact structure/indentation.

- [ ] **Step 2: Add a "Test archive" subcommand to each archive-extension block**

For **every** archive extension already present (`.zip .tar .gz .tgz .bz2 .xz .zst .7z .rar`), the existing block has subcommands `01here`, `02to`, `03open` under `...\shell\rza`. Insert a **Test** entry and renumber so order is Extract Here → Extract to → Test → Open. For the `.zip` block it becomes (apply the same to each extension, substituting the extension):

```nsi
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\02to" "" "Extract to subfolder"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\03test" "" "Test archive"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\03test\command" "" '"$INSTDIR\rza.exe" test "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\04open" "" "Open with rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\04open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'
```

- [ ] **Step 3: Replace the flat `*\shell\rza.add` with a cascading submenu for `*` and `Directory`**

Remove the old single `*\shell\rza.add` (+ its command) and add a cascading **rza** submenu for both `*` (all files) and `Directory` (folders). Add this block (the `*` variant shown; duplicate it replacing `*` with `Directory`):

```nsi
  WriteRegStr HKCU "Software\Classes\*\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\*\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\01add" "" "Add to archive…"
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\01add\command" "" '"$INSTDIR\rza-gui.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\02zip" "" "Compress to .zip"
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\02zip\command" "" '"$INSTDIR\rza.exe" compress-zip "%1"'
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\03targz" "" "Compress to .tar.gz"
  WriteRegStr HKCU "Software\Classes\*\shell\rza\shell\03targz\command" "" '"$INSTDIR\rza.exe" compress-targz "%1"'
```

- [ ] **Step 4: Update the uninstall macro**

In `RzaUninstallShellMenu`, replace the old `DeleteRegKey HKCU "Software\Classes\*\shell\rza.add"` with:

```nsi
  DeleteRegKey HKCU "Software\Classes\*\shell\rza"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\rza"
```

(The per-extension `DeleteRegKey ...\shell\rza` lines already cover the added Test subkey, since deleting the `rza` key removes all its `shell\*` children.)

- [ ] **Step 5: Mirror every change into `Cargo.toml`'s `preinstall-section`**

The `preinstall-section` string in `[package.metadata.packager]` duplicates the install macro body. Apply the **same** additions there, escaping backslashes as `\\` for TOML (each `\` in the `.nsh` becomes `\\`). Keep the two copies identical in content. (The uninstall macro is only in the `.nsh` for reference; cargo-packager 0.11.8 doesn't wire it — unchanged from before.)

- [ ] **Step 6: Verify the Linux package still builds (config has no parse error)**

Run:
```
source "$HOME/.cargo/env"
cargo build --release --features gui
cargo packager --release --formats deb
find target dist -name '*.deb' | head -1
```
Expected: a `.deb` is produced (the TOML `preinstall-section` still parses). The Windows menu itself is not testable here — note in your report that it needs manual Windows verification.

- [ ] **Step 7: Commit**

```bash
git add packaging/windows/shell-menu.nsh Cargo.toml
git commit -m "feat(packaging): add Test + Compress entries to the shell menu"
```

---

### Task 3: Docs

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update the "### Right-click menu" subsection in `README.md`**

Replace the bullet list in that subsection with the expanded actions:

```markdown
- **Windows:** right-click an archive → (Windows 11: **Show more options** →)
  **rza ▸** → **Extract Here**, **Extract to subfolder**, **Test archive**, or
  **Open with rza**. Right-click any file or folder → **rza ▸** → **Add to
  archive…**, **Compress to .zip**, or **Compress to .tar.gz**.
- **Linux:** right-click an archive → **Open With → rza — Archive Utility**.

These call the same engine as the CLI (`rza extract-here`, `rza test`,
`rza compress-zip`, …). Menu labels are fixed text; the created/extracted files
are still named after the item.
```

Keep the existing uninstall-limitation note line.

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: document the expanded right-click menu"
```

---

## Self-Review Notes

- **Spec coverage:** CLI `test`/`compress-zip`/`compress-targz` + `archive_output` helper with folder/stem/no-ext/two-part-ext/empty-parent cases (Task 1); registry Test entry on archives + cascading Compress submenu for `*` and `Directory` + uninstall macro update, mirrored into Cargo.toml (Task 2); README (Task 3). Static-label decision, per-user keys, and the accepted uninstall limitation are reflected.
- **Placeholder scan:** Task 1 has complete code/tests. Task 2 gives the exact `.zip` block and the `*` block and explicitly says to apply the same to each of the 9 extensions / to `Directory` — mechanical repetition the registry forces, not a vague placeholder; the implementer reads the existing file first (Step 1) to match structure.
- **Type consistency:** `archive_output(&Path, &str) -> PathBuf` defined and tested in Task 1, used by both compress handlers; the three `Command` variants' names (`Test`/`CompressZip`/`CompressTargz`) match between cli.rs and the rza.rs handlers; the registry commands invoke exactly `test`/`compress-zip`/`compress-targz` (clap kebab-cases the variant names).
- **Verification reality (flag to human):** Tasks 1 & 3 fully verified on Linux. Task 2's registry menu is verified only by installing the CI build on Windows; local bar is "`.deb` builds, config parses."
- **clap naming check:** clap derives the subcommand name by kebab-casing the variant, so `CompressZip` → `compress-zip`, `CompressTargz` → `compress-targz`, `Test` → `test` — matching the registry commands. (If the implementer finds clap produced a different name, add `#[command(name = "...")]` to align; note it.)
