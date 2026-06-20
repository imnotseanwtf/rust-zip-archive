# Windows Shell Context Menu (Slice C) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a 7-Zip-style right-click **rza ▸** submenu in Windows Explorer (Extract Here / Extract to "name\" / Open with rza / Add to archive…), backed by new CLI subcommands and GUI multi-file launch, plus a Linux "Open With" association.

**Architecture:** New CLI subcommands `extract-here`/`extract-to` (pure destination-deriving helpers + thin wrappers over `archive::extract`). `rza-gui` gains a `launch_intent` that stages multiple/non-archive paths for create mode. The Windows installer (cargo-packager + a custom NSIS hook) writes per-user registry `SubCommands` keys. The shell menu itself is verified manually on Windows.

**Tech Stack:** Rust 2021; existing archive library + clap CLI + eframe GUI; cargo-packager / NSIS for the installer registry keys.

## Global Constraints

- Edition 2021; Linux/macOS/Windows.
- `cargo build` (no features) stays CLI-only; `gui` opt-in.
- Registry entries are **per-user** (`HKCU\Software\Classes`), added on install and removed on uninstall.
- Windows 11 shows the menu under "Show more options" (registry menus) — accepted/documented.
- CLI/GUI parts are verified automatically on Linux; the **registry menu is verified manually by the user on Windows** (build-in-CI + install-and-check).
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo clippy --features gui --bin rza-gui -- -D warnings` stay clean.

---

### Task 1: CLI `extract-here` and `extract-to`

Add two shell-callable subcommands that derive a destination from the archive path and extract.

**Files:**
- Modify: `src/cli.rs` (two new `Command` variants)
- Modify: `src/bin/rza.rs` (helpers + handlers + inline unit tests)
- Test: `tests/shell_cli.rs`

**Interfaces:**
- Produces (in `src/bin/rza.rs`):
  - `fn extract_here_dir(archive: &Path) -> PathBuf` — the archive's own parent dir.
  - `fn extract_to_dir(archive: &Path) -> PathBuf` — parent dir joined with the file name minus a recognized archive/compressor suffix (fallback `<name>_extracted`).
- Produces (in `src/cli.rs`): `Command::ExtractHere { archive: PathBuf }`, `Command::ExtractTo { archive: PathBuf }`.

- [ ] **Step 1: Write the unit tests at the bottom of `src/bin/rza.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::{extract_here_dir, extract_to_dir};
    use std::path::{Path, PathBuf};

    #[test]
    fn here_dir_is_parent() {
        assert_eq!(extract_here_dir(Path::new("/tmp/a/b.zip")), PathBuf::from("/tmp/a"));
    }

    #[test]
    fn to_dir_strips_known_suffixes() {
        assert_eq!(extract_to_dir(Path::new("/tmp/a/b.zip")), PathBuf::from("/tmp/a/b"));
        assert_eq!(extract_to_dir(Path::new("/tmp/a/b.tar.gz")), PathBuf::from("/tmp/a/b"));
        assert_eq!(extract_to_dir(Path::new("/tmp/a/b.tgz")), PathBuf::from("/tmp/a/b"));
    }

    #[test]
    fn to_dir_fallback_when_no_known_suffix() {
        assert_eq!(extract_to_dir(Path::new("/tmp/a/weird.bin")), PathBuf::from("/tmp/a/weird.bin_extracted"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --bin rza`
Expected: FAIL to compile — helpers not defined.

- [ ] **Step 3: Add the two `Command` variants in `src/cli.rs`**

Add inside the `enum Command { ... }` (after `List`):

```rust
    /// Extract an archive into the folder that contains it (for shell "Extract Here").
    ExtractHere {
        /// Archive to extract.
        archive: PathBuf,
    },

    /// Extract an archive into a new subfolder named after it (for shell "Extract to name\").
    ExtractTo {
        /// Archive to extract.
        archive: PathBuf,
    },
```

- [ ] **Step 4: Add the helpers + handlers in `src/bin/rza.rs`**

Add the helpers (near the top, after the `use` lines):

```rust
use std::path::{Path, PathBuf};

/// Destination for "Extract Here": the archive's own directory.
fn extract_here_dir(archive: &Path) -> PathBuf {
    archive
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

/// Destination for "Extract to name\\": parent dir + the file name with a
/// recognized archive/compressor suffix removed (fallback `<name>_extracted`).
fn extract_to_dir(archive: &Path) -> PathBuf {
    const SUFFIXES: &[&str] = &[
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tgz", ".tbz2", ".txz", ".tzst", ".tar",
        ".zip", ".7z", ".rar", ".gz", ".bz2", ".xz", ".zst",
    ];
    let parent = archive
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let fname = archive
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let lower = fname.to_lowercase();
    let stem = SUFFIXES
        .iter()
        .find(|s| lower.ends_with(*s))
        .map(|s| fname[..fname.len() - s.len()].to_string())
        .unwrap_or_else(|| format!("{fname}_extracted"));
    parent.join(stem)
}
```

In `main`, add match arms for the two commands (reuse the existing `make_bar` + `archive::extract` pattern used by `Command::Extract`):

```rust
        Command::ExtractHere { archive } => {
            let dest = extract_here_dir(&archive);
            let bar = make_bar("Extracting");
            rust_zip_archive::archive::extract(&archive, &dest, false, |p: Progress| {
                bar.set_length(p.total);
                bar.set_position(p.current);
                bar.set_message(p.message);
            })?;
            bar.finish_with_message(format!("Extracted into {}", dest.display()));
        }

        Command::ExtractTo { archive } => {
            let dest = extract_to_dir(&archive);
            let bar = make_bar("Extracting");
            rust_zip_archive::archive::extract(&archive, &dest, false, |p: Progress| {
                bar.set_length(p.total);
                bar.set_position(p.current);
                bar.set_message(p.message);
            })?;
            bar.finish_with_message(format!("Extracted into {}", dest.display()));
        }
```

(If `Path`/`PathBuf` are already imported in `rza.rs`, don't duplicate the `use`.)

- [ ] **Step 5: Run unit tests to verify they pass**

Run: `source "$HOME/.cargo/env" && cargo test --bin rza`
Expected: the 3 helper tests pass.

- [ ] **Step 6: Write the integration test (`tests/shell_cli.rs`)**

```rust
use std::process::Command;

fn rza() -> Command { Command::new(env!("CARGO_BIN_EXE_rza")) }

#[test]
fn extract_here_lands_in_archive_dir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = root.join("data.txt");
    std::fs::write(&src, "hello\n").unwrap();
    let archive = root.join("bundle.zip");
    assert!(rza().arg("create").arg("-o").arg(&archive).arg(&src).status().unwrap().success());

    assert!(rza().arg("extract-here").arg(&archive).status().unwrap().success());
    // "Extract Here" puts entries directly in the archive's folder.
    assert!(root.join("data.txt").exists());
}

#[test]
fn extract_to_lands_in_named_subfolder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = root.join("data.txt");
    std::fs::write(&src, "hello\n").unwrap();
    let archive = root.join("bundle.zip");
    assert!(rza().arg("create").arg("-o").arg(&archive).arg(&src).status().unwrap().success());

    assert!(rza().arg("extract-to").arg(&archive).status().unwrap().success());
    // "Extract to bundle\" puts entries under a subfolder named after the archive.
    assert!(root.join("bundle").join("data.txt").exists());
}
```

- [ ] **Step 7: Run all tests + lint**

Run: `source "$HOME/.cargo/env" && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: new tests pass plus all prior; clippy + fmt clean.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(cli): extract-here and extract-to subcommands for shell menu"
```

---

### Task 2: GUI multi-file launch (Add to archive…)

Teach `rza-gui` to stage multiple/non-archive paths for create mode when launched with them.

**Files:**
- Modify: `src/bin/rza-gui/main.rs` (add `launch_intent` + tests, wire into `main`)
- Modify: `src/bin/rza-gui/app.rs` (add `stage_paths`)

**Interfaces:**
- Produces (in `main.rs`):
  ```rust
  enum Launch { Open(std::path::PathBuf), Stage(Vec<std::path::PathBuf>), Empty }
  fn launch_intent(args: &[String]) -> Launch;
  ```
  Single existing file that `archive::format::detect_for_read` accepts → `Open`; one-or-more existing paths otherwise (multiple, a directory, or a non-archive file) → `Stage`; nothing usable → `Empty`.
- Produces (in `app.rs`): `pub(crate) fn stage_paths(&mut self, paths: Vec<PathBuf>)` — extends `self.staged` and sets a status string.

- [ ] **Step 1: Add `stage_paths` to `src/bin/rza-gui/app.rs`**

In `impl RzaApp`:

```rust
    pub(crate) fn stage_paths(&mut self, paths: Vec<PathBuf>) {
        self.staged.extend(paths);
        self.status = format!("{} file(s) staged", self.staged.len());
    }
```

- [ ] **Step 2: Write the failing tests at the bottom of `src/bin/rza-gui/main.rs`**

Add to the existing `#[cfg(test)] mod tests`:

```rust
    use super::{launch_intent, Launch};

    #[test]
    fn single_archive_opens() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.zip");
        // Minimal valid empty zip (PK end-of-central-directory record).
        std::fs::write(&f, [0x50,0x4B,0x05,0x06,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]).unwrap();
        let args = vec!["rza-gui".to_string(), f.to_string_lossy().to_string()];
        assert!(matches!(launch_intent(&args), Launch::Open(_)));
    }

    #[test]
    fn multiple_paths_stage() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt"); std::fs::write(&a, "a").unwrap();
        let b = dir.path().join("b.txt"); std::fs::write(&b, "b").unwrap();
        let args = vec!["rza-gui".into(), a.to_string_lossy().into(), b.to_string_lossy().into()];
        match launch_intent(&args) {
            Launch::Stage(v) => assert_eq!(v.len(), 2),
            _ => panic!("expected Stage"),
        }
    }

    #[test]
    fn no_args_is_empty() {
        assert!(matches!(launch_intent(&["rza-gui".to_string()]), Launch::Empty));
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --features gui --bin rza-gui`
Expected: FAIL to compile — `launch_intent`/`Launch` not defined.

- [ ] **Step 4: Implement `launch_intent` and use it in `main`**

In `main.rs`, add (replacing the `first_existing_file` usage in `main`):

```rust
use std::path::PathBuf;

/// What to do with the paths the app was launched with.
pub enum Launch {
    Open(PathBuf),
    Stage(Vec<PathBuf>),
    Empty,
}

/// A single existing, recognizable archive → open it; one or more other existing
/// paths → stage them for a new archive; nothing usable → empty.
pub fn launch_intent(args: &[String]) -> Launch {
    let existing: Vec<PathBuf> = args
        .iter()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();
    match existing.as_slice() {
        [] => Launch::Empty,
        [one]
            if one.is_file()
                && rust_zip_archive::archive::format::detect_for_read(one).is_ok() =>
        {
            Launch::Open(one.clone())
        }
        _ => Launch::Stage(existing),
    }
}
```

Replace `main`'s body that built the app so it routes the intent:

```rust
    eframe::run_native(
        "rza — Archive Utility",
        options,
        Box::new(move |_cc| {
            let mut app = RzaApp::default();
            match intent {
                Launch::Open(p) => app.open_archive(p),
                Launch::Stage(paths) => app.stage_paths(paths),
                Launch::Empty => {}
            }
            Ok(Box::new(app))
        }),
    )
```

…where, earlier in `main`, replace `let initial = first_existing_file(&args);` with `let intent = launch_intent(&args);`. Remove the now-unused `first_existing_file` (and its old tests) ONLY if nothing else uses it; if its tests are still wanted, keep the function. (Clippy will flag it if dead.)

- [ ] **Step 5: Run tests + lint**

Run: `source "$HOME/.cargo/env" && cargo test --features gui --bin rza-gui && cargo clippy --features gui --bin rza-gui -- -D warnings && cargo fmt --all -- --check`
Expected: the 3 launch tests pass (plus any kept); clippy + fmt clean.

- [ ] **Step 6: Confirm CLI-only build unaffected + commit**

```bash
cargo build
git add -A
git commit -m "feat(gui): stage multiple/non-archive paths for create mode (Add to archive)"
```

---

### Task 3: Installer registry menu (Windows) + Linux MimeType

Make the installer register the cascading **rza ▸** submenu on Windows and add the Linux `MimeType`. **The Windows menu is verified manually by the user on Windows; this task's local bar is that the config/hook is in place and the Linux package still builds.**

**Files:**
- Create: `packaging/windows/shell-menu.nsh` (NSIS include with the registry writes/removals)
- Modify: `Cargo.toml` (`[package.metadata.packager]` — NSIS hook + Linux mime types)

**Interfaces:**
- Consumes: installed `rza.exe` / `rza-gui.exe`; the CLI subcommands from Task 1; the GUI launch from Task 2.

- [ ] **Step 1: Research the installed cargo-packager's NSIS customization**

Run: `source "$HOME/.cargo/env" && cargo packager --help` and read the installed version's docs for: (a) how to inject custom NSIS script (an `nsis` `installer-hooks`/`template`/`custom-*` key under the packager config), and (b) how Linux file-association `mime-type`s are declared. Record the exact keys the installed version accepts in your report; ADAPT Steps 2–3 to them.

- [ ] **Step 2: Create `packaging/windows/shell-menu.nsh`**

NSIS macros that add the per-user cascading submenu on install and remove it on uninstall. `$INSTDIR` is the install directory cargo-packager/NSIS provides.

```nsi
; rza shell context menu — per-user (HKCU), cascading "rza" submenu.
!macro RzaInstallShellMenu
  ; --- Archive types: cascading submenu with extract/open ---
  ; Apply to each archive extension's ProgId-less class under HKCU\Software\Classes\<.ext>
  !define RZA_EXTS ".zip .tar .gz .tgz .bz2 .xz .zst .7z .rar"
  ; (NSIS lacks a foreach; the implementer expands one block per extension, OR
  ;  registers on the SystemFileAssociations\<ext> class. Example for .zip:)
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza" "MUIVerb" "rza"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza" "SubCommands" ""
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\01here\command" "" '"$INSTDIR\rza.exe" extract-here "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\01here" "" "Extract Here"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\02to\command" "" '"$INSTDIR\rza.exe" extract-to "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\02to" "" 'Extract to folder'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\03open\command" "" '"$INSTDIR\rza-gui.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza\shell\03open" "" "Open with rza"

  ; --- All files: Add to archive ---
  WriteRegStr HKCU "Software\Classes\*\shell\rza.add" "" "Add to archive (rza)…"
  WriteRegStr HKCU "Software\Classes\*\shell\rza.add\command" "" '"$INSTDIR\rza-gui.exe" "%1"'
!macroend

!macro RzaUninstallShellMenu
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.zip\shell\rza"
  DeleteRegKey HKCU "Software\Classes\*\shell\rza.add"
!macroend
```

The implementer expands the per-extension block for each of `.zip .tar .gz .tgz .bz2 .xz .zst .7z .rar` (same body, different extension), and calls `!insertmacro RzaInstallShellMenu` from the packager's NSIS install hook and `RzaUninstallShellMenu` from the uninstall hook (wiring per Step 1's findings).

- [ ] **Step 3: Wire the hook + Linux mime types in `Cargo.toml`**

Under `[package.metadata.packager]`, add the NSIS hook reference (exact key per Step 1) pointing at `packaging/windows/shell-menu.nsh`, and add Linux mime types to the existing file-association config so the `.desktop` gets `MimeType=` (e.g. `application/zip`, `application/x-tar`, `application/gzip`, `application/x-xz`, `application/zstd`, `application/x-bzip2`, `application/x-7z-compressed`, `application/vnd.rar`). Record the exact final config in your report.

- [ ] **Step 4: Verify the Linux package still builds with the new config**

Run:
```
source "$HOME/.cargo/env"
cargo build --release --features gui
cargo packager --release --formats deb
```
Then confirm the `.desktop` now carries `MimeType=`:
```
DEB=$(find target dist -name '*.deb' | head -1)
dpkg -x "$DEB" /tmp/rzadeb && grep -R "MimeType=" /tmp/rzadeb || echo "NO MimeType (investigate config key)"
```
Expected: a `.deb` builds and its `.desktop` contains a `MimeType=` line. If cargo-packager doesn't emit `MimeType` from the config, note it; the minimum local bar is the `.deb` builds without a config error.
**Windows note in your report:** the registry submenu cannot be tested here — state that it requires installing the CI-built `.exe` on Windows and checking Explorer → (Win11: "Show more options" →) rza ▸.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(packaging): Windows shell context menu + Linux MimeType association"
```

---

### Task 4: Docs — right-click menu

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add a "Right-click menu" subsection under "Install the app" in `README.md`**

```markdown
### Right-click menu

After installing, archives get an **rza** menu in your file manager:

- **Windows:** right-click an archive → (on Windows 11, **Show more options** →)
  **rza ▸** → **Extract Here**, **Extract to folder**, or **Open with rza**.
  Right-click any file/folder → **Add to archive (rza)…** opens the app ready to
  create a new archive.
- **Linux:** right-click an archive → **Open With → rza — Archive Utility**.

These call the same engine as the CLI (`rza extract-here` / `rza extract-to`).
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: document the right-click menu"
```

---

## Self-Review Notes

- **Spec coverage:** CLI `extract-here`/`extract-to` + pure dir helpers with fallback (Task 1); GUI `launch_intent` multi-file staging for "Add to archive" (Task 2); installer registry SubCommands menu + per-user keys + uninstall removal + Linux MimeType (Task 3); docs incl. the Win11 "Show more options" note (Task 4).
- **Placeholder scan:** Tasks 1–2 and 4 are concrete. Task 3 deliberately contains research-and-adapt steps because cargo-packager's NSIS-hook and mime-type config keys vary by version and cannot be pinned blind; the NSH macro body is concrete, with explicit instructions to expand per-extension and wire via the version's hook key. This is the inherent, pre-disclosed risk of the Windows-only seam, not an avoidable placeholder.
- **Type consistency:** `extract_here_dir`/`extract_to_dir` defined and tested in Task 1, called by the same file's handlers; `Launch`/`launch_intent` defined in Task 2 `main.rs`, `stage_paths` defined in `app.rs` and called by `main`; registry commands invoke exactly the `extract-here`/`extract-to` verbs from Task 1 and `rza-gui "%1"` from Task 2.
- **Verification reality (flag to human):** Tasks 1–2 are fully testable on Linux (unit + integration). Task 3's registry menu is **not testable in this environment** — local bar is "config builds + Linux MimeType present"; the Windows menu requires the user to install the CI build and confirm, with likely iteration on the NSIS hook.
