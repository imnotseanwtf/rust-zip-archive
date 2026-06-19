# GUI Redesign (7-Zip-style) + Windows Console Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `rza-gui` as a 7-Zip-style file manager (folder navigation, toolbar, resizable columns, in-window right-click, integrity Test) and stop the Windows console window from appearing behind it.

**Architecture:** Split the GUI binary into `src/bin/rza-gui/{main.rs,app.rs,tree.rs}`. A pure `tree` model turns the library's flat entry list into navigable folders. The view uses `egui_extras::TableBuilder`. A new library `test()` powers the integrity action. The existing background-job/progress machinery and `archive::*` API are reused.

**Tech Stack:** Rust 2021; `eframe`/`egui` 0.29 + new `egui_extras` 0.29 (gui feature); existing archive library; `tempfile` (dev).

## Global Constraints

- Edition 2021; Linux/macOS/Windows.
- `cargo build` (no features) stays CLI-only; the `gui` feature stays opt-in; `egui_extras` is an optional dep under `gui`.
- Release builds of the GUI must not open a console window on Windows (`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`).
- Reuse existing `archive::{create,extract,extract_selected,list}`, `Progress`, `EntryInfo`, the `spawn_job`/`JobMsg`/`poll_job` machinery, drag-and-drop, and argv open-on-launch — do not rebuild them.
- No in-archive editing (delete/rename/add-to-existing); no Explorer shell menu; no Modified column; glyphs not image icons; no column sorting.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo clippy --features gui --bin rza-gui -- -D warnings` stay clean.

---

### Task 1: Windows console fix + split the GUI binary into modules

Relocate the single-file GUI binary into a directory binary and add the console-suppression attribute. No UI behavior change.

**Files:**
- Create: `src/bin/rza-gui/main.rs` (entry point)
- Create: `src/bin/rza-gui/app.rs` (all current `RzaApp` code, relocated)
- Delete: `src/bin/rza-gui.rs`
- Modify: `Cargo.toml` (`[[bin]]` path for `rza-gui`)

**Interfaces:**
- Produces: `app::RzaApp` (`pub(crate)`), `app::RzaApp::open_archive(&mut self, path: PathBuf)` (`pub(crate)`), and `app::RzaApp::default()`. `main.rs` keeps `first_existing_file`.

- [ ] **Step 1: Update the `[[bin]]` path in `Cargo.toml`**

Change the `rza-gui` binary entry to the directory layout:

```toml
[[bin]]
name = "rza-gui"
path = "src/bin/rza-gui/main.rs"
required-features = ["gui"]
```

- [ ] **Step 2: Create `src/bin/rza-gui/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use app::RzaApp;
use eframe::egui;

/// The first CLI argument (after the program name) that names an existing file.
fn first_existing_file(args: &[String]) -> Option<std::path::PathBuf> {
    args.iter()
        .skip(1)
        .map(std::path::PathBuf::from)
        .find(|p| p.is_file())
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let initial = first_existing_file(&args);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rza — Archive Utility",
        options,
        Box::new(move |_cc| {
            let mut app = RzaApp::default();
            if let Some(path) = initial {
                app.open_archive(path);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::first_existing_file;
    use std::path::PathBuf;

    #[test]
    fn picks_first_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.zip");
        std::fs::write(&f, b"x").unwrap();
        let args = vec!["rza-gui".to_string(), f.to_string_lossy().to_string()];
        assert_eq!(first_existing_file(&args), Some(PathBuf::from(&f)));
    }

    #[test]
    fn none_when_arg_missing_or_absent() {
        let args = vec!["rza-gui".to_string(), "/no/such/file.zip".to_string()];
        assert_eq!(first_existing_file(&args), None);
        assert_eq!(first_existing_file(&["rza-gui".to_string()]), None);
    }
}
```

- [ ] **Step 3: Create `src/bin/rza-gui/app.rs` from the old binary's body**

Move everything from the old `src/bin/rza-gui.rs` EXCEPT `main` and `first_existing_file` (and its `#[cfg(test)]` tests) into `src/bin/rza-gui/app.rs`: the `use` lines it needs, `struct RzaApp`, its `Default` impl, `impl RzaApp { ... }`, `impl eframe::App for RzaApp`, and the helper types (`Row`, `JobMsg`, etc.). Make `RzaApp` and its `open_archive` method `pub(crate)`. Keep all logic identical. Top of `app.rs` will look like:

```rust
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use eframe::egui;
use rust_zip_archive::archive::{self, EntryInfo, Progress};
```

(Adjust the `use` list to exactly what the moved code references; `cargo build --features gui` will tell you if anything is missing or unused.)

- [ ] **Step 4: Delete the old file**

`git rm src/bin/rza-gui.rs`

- [ ] **Step 5: Build, test, lint**

Run:
```
source "$HOME/.cargo/env"
cargo build --features gui
cargo test --features gui --bin rza-gui
cargo clippy --features gui --bin rza-gui -- -D warnings
cargo build
```
Expected: GUI builds; the 2 argv tests pass; clippy clean; CLI-only build unaffected.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(gui): split into modules + suppress Windows console window"
```

---

### Task 2: Library `test()` integrity check

Add a per-format integrity check that reads every entry through a sink.

**Files:**
- Modify: `src/archive/mod.rs` (public `test` + dispatch)
- Modify: `src/archive/zip.rs`, `src/archive/tar.rs`, `src/archive/compressor.rs` (per-backend `test`)
- Test: `tests/integrity.rs`

**Interfaces:**
- Produces (in `mod.rs`): `pub fn test(archive: &Path, progress: impl FnMut(Progress)) -> Result<()>`.
- Produces (each backend, `pub(crate)`): `fn test(archive: &Path, [format: Format,] progress: impl FnMut(Progress)) -> Result<()>` (tar/compressor take `format`; zip does not).

- [ ] **Step 1: Write the failing tests (`tests/integrity.rs`)**

```rust
use rust_zip_archive::archive;
use rust_zip_archive::cli::Compression;
use std::fs;
use std::path::Path;

fn write(path: &Path, contents: &str) {
    if let Some(p) = path.parent() { fs::create_dir_all(p).unwrap(); }
    fs::write(path, contents).unwrap();
}

fn good_archive(ext: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("sample");
    write(&src.join("a.txt"), &"data\n".repeat(100));
    let archive = dir.path().join(format!("a{ext}"));
    archive::create(&archive, std::slice::from_ref(&src), Compression::Deflate, false, |_p| {}).unwrap();
    (dir, archive)
}

#[test]
fn test_passes_on_good_archives() {
    for ext in [".zip", ".tar.gz", ".gz"] {
        let (_d, archive) = if ext == ".gz" {
            // single-file compressor needs a single file input
            let dir = tempfile::tempdir().unwrap();
            let f = dir.path().join("x.txt");
            write(&f, &"y".repeat(200));
            let a = dir.path().join("x.txt.gz");
            archive::create(&a, std::slice::from_ref(&f), Compression::Deflate, false, |_p| {}).unwrap();
            (dir, a)
        } else {
            good_archive(ext)
        };
        archive::test(&archive, |_p| {}).expect(&format!("{ext} should pass"));
    }
}

#[test]
fn test_fails_on_corrupt_archive() {
    let (_d, archive) = good_archive(".zip");
    // Truncate the file to corrupt it.
    let data = fs::read(&archive).unwrap();
    fs::write(&archive, &data[..data.len() / 2]).unwrap();
    assert!(archive::test(&archive, |_p| {}).is_err(), "truncated zip should fail test");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --test integrity`
Expected: FAIL — `archive::test` not found.

- [ ] **Step 3: Add `test` to `src/archive/zip.rs`**

```rust
pub(crate) fn test(archive: &Path, mut progress: impl FnMut(Progress)) -> Result<()> {
    let file =
        File::open(archive).with_context(|| format!("opening archive {}", archive.display()))?;
    let mut zip = ZipArchive::new(BufReader::new(file))
        .with_context(|| format!("reading archive {}", archive.display()))?;
    let total = zip.len() as u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        progress(Progress { current: i as u64, total, message: name.clone() });
        // Reading the whole entry validates the CRC (zip checks it on read end).
        io::copy(&mut entry, &mut io::sink())
            .with_context(|| format!("verifying {name}"))?;
    }
    progress(Progress { current: total, total, message: "ok".into() });
    Ok(())
}
```

- [ ] **Step 4: Add `test` to `src/archive/tar.rs`**

Reuse the existing `with_reader` helper:

```rust
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
            progress(Progress { current: idx, total: 0, message: name.clone() });
            idx += 1;
            std::io::copy(&mut entry, &mut std::io::sink())
                .with_context(|| format!("verifying {name}"))?;
        }
        progress(Progress { current: idx, total: idx, message: "ok".into() });
        Ok(())
    })
}
```

- [ ] **Step 5: Add `test` to `src/archive/compressor.rs`**

```rust
pub(crate) fn test(
    archive: &Path,
    format: Format,
    mut progress: impl FnMut(Progress),
) -> Result<()> {
    progress(Progress { current: 0, total: 1, message: inner_name(archive, format) });
    let mut dec = open_decoder(archive, format)?;
    std::io::copy(&mut dec, &mut std::io::sink())
        .with_context(|| format!("verifying {}", archive.display()))?;
    progress(Progress { current: 1, total: 1, message: "ok".into() });
    Ok(())
}
```

- [ ] **Step 6: Add the public `test` dispatcher to `src/archive/mod.rs`**

```rust
pub fn test(archive: &Path, progress: impl FnMut(Progress)) -> Result<()> {
    let format = format::detect_for_read(archive)?;
    match format {
        Format::Zip => zip::test(archive, progress),
        Format::Tar | Format::TarGz | Format::TarBz2 | Format::TarXz | Format::TarZst => {
            tar::test(archive, format, progress)
        }
        Format::Gz | Format::Bz2 | Format::Xz | Format::Zst => {
            compressor::test(archive, format, progress)
        }
        other => bail!("testing {:?} archives is not supported yet", other),
    }
}
```

- [ ] **Step 7: Run tests + lint**

Run: `source "$HOME/.cargo/env" && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: `tests/integrity.rs` (2) pass plus all prior; clippy + fmt clean.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: archive integrity test() across all formats"
```

---

### Task 3: Archive folder-tree model (`tree.rs`)

A pure model that turns the flat entry list into navigable folders.

**Files:**
- Create: `src/bin/rza-gui/tree.rs`
- Modify: `src/bin/rza-gui/app.rs` (add `mod tree;` — actually declare in `main.rs`)
- Modify: `src/bin/rza-gui/main.rs` (add `mod tree;`)

**Interfaces:**
- Produces (in `tree.rs`):
  ```rust
  pub struct Node { pub name: String, pub full_path: String, pub is_dir: bool, pub size: u64, pub compressed: u64 }
  pub fn children(entries: &[EntryInfo], dir: &str) -> Vec<Node>;
  ```
  `dir` is a slash path ("" = root). Returns immediate children, folders first then files, each group sorted by name. Intermediate folders not present as explicit entries are synthesized.

- [ ] **Step 1: Write the failing tests at the bottom of `src/bin/rza-gui/tree.rs`**

Create the file with the tests first:

```rust
use rust_zip_archive::archive::EntryInfo;

// (implementation added in Step 3)

#[cfg(test)]
mod tests {
    use super::children;
    use rust_zip_archive::archive::EntryInfo;

    fn e(name: &str, is_dir: bool) -> EntryInfo {
        EntryInfo { name: name.to_string(), size: 10, compressed: 5, is_dir }
    }

    #[test]
    fn root_lists_top_level_folders_first() {
        let entries = vec![e("top.txt", false), e("a/b.txt", false), e("a/c/d.txt", false)];
        let kids = children(&entries, "");
        let names: Vec<_> = kids.iter().map(|n| (n.name.as_str(), n.is_dir)).collect();
        assert_eq!(names, vec![("a", true), ("top.txt", false)]);
        assert_eq!(kids[0].full_path, "a");
    }

    #[test]
    fn subdir_lists_its_children() {
        let entries = vec![e("a/b.txt", false), e("a/c/d.txt", false)];
        let kids = children(&entries, "a");
        let names: Vec<_> = kids.iter().map(|n| (n.name.as_str(), n.is_dir)).collect();
        assert_eq!(names, vec![("c", true), ("b.txt", false)]);
        assert_eq!(kids[0].full_path, "a/c");
    }

    #[test]
    fn explicit_dir_entries_are_not_duplicated() {
        let entries = vec![e("a/", true), e("a/b.txt", false)];
        let kids = children(&entries, "");
        assert_eq!(kids.len(), 1);
        assert_eq!((kids[0].name.as_str(), kids[0].is_dir), ("a", true));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

First wire the module so it compiles: add `mod tree;` to `src/bin/rza-gui/main.rs` (after `mod app;`).
Run: `source "$HOME/.cargo/env" && cargo test --features gui --bin rza-gui`
Expected: FAIL — `children` not defined.

- [ ] **Step 3: Implement the model at the top of `src/bin/rza-gui/tree.rs`**

```rust
use rust_zip_archive::archive::EntryInfo;
use std::collections::BTreeMap;

/// One row in the file-manager view: a file or a (possibly synthesized) folder.
pub struct Node {
    pub name: String,
    pub full_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub compressed: u64,
}

/// Immediate children of `dir` ("" = root): folders first, then files, each
/// group sorted by name. Folders are synthesized from path prefixes.
pub fn children(entries: &[EntryInfo], dir: &str) -> Vec<Node> {
    let prefix = if dir.is_empty() { String::new() } else { format!("{dir}/") };

    let mut dirs: BTreeMap<String, ()> = BTreeMap::new();
    let mut files: Vec<Node> = Vec::new();

    for e in entries {
        // Normalize trailing slash on explicit directory entries.
        let path = e.name.trim_end_matches('/');
        if path.is_empty() {
            continue;
        }
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            continue; // the dir entry itself
        }
        match rest.split_once('/') {
            Some((first, _)) => {
                // `first` is a subfolder directly under `dir`.
                dirs.insert(first.to_string(), ());
            }
            None => {
                if e.name.ends_with('/') {
                    dirs.insert(rest.to_string(), ());
                } else {
                    files.push(Node {
                        name: rest.to_string(),
                        full_path: format!("{prefix}{rest}"),
                        is_dir: false,
                        size: e.size,
                        compressed: e.compressed,
                    });
                }
            }
        }
    }

    let mut out: Vec<Node> = dirs
        .into_keys()
        .map(|name| Node {
            full_path: format!("{prefix}{name}"),
            name,
            is_dir: true,
            size: 0,
            compressed: 0,
        })
        .collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend(files);
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `source "$HOME/.cargo/env" && cargo test --features gui --bin rza-gui`
Expected: the 3 tree tests + 2 argv tests pass.

- [ ] **Step 5: Lint + commit**

```bash
cargo clippy --features gui --bin rza-gui -- -D warnings && cargo fmt --all
git add -A
git commit -m "feat(gui): archive folder-tree model"
```

---

### Task 4: File-manager view — navigation + resizable table

Replace the flat checkbox grid with a navigable `egui_extras` table driven by the tree model. Keep the existing Open/Extract controls working.

**Files:**
- Modify: `Cargo.toml` (add optional `egui_extras` under `gui`)
- Modify: `src/bin/rza-gui/app.rs`

**Interfaces:**
- Consumes: `tree::{children, Node}`, existing `RzaApp` fields and job machinery.
- Produces: nav state on `RzaApp` (`current_dir: String`, `selected: std::collections::HashSet<String>`) used by Task 5.

- [ ] **Step 1: Add `egui_extras` to `Cargo.toml`**

In `[dependencies]`:

```toml
egui_extras = { version = "0.29", optional = true, default-features = false }
```

Add it to the `gui` feature:

```toml
gui = ["dep:eframe", "dep:egui", "dep:rfd", "dep:egui_extras"]
```

- [ ] **Step 2: Add navigation state to `RzaApp`**

In `app.rs`, add fields to `RzaApp`:

```rust
    current_dir: String,
    selected: std::collections::HashSet<String>,
```

Initialize them in the `Default` impl (`current_dir: String::new(), selected: std::collections::HashSet::new()`). In `open_archive`, after loading entries, reset navigation:

```rust
        self.current_dir.clear();
        self.selected.clear();
```

- [ ] **Step 3: Replace the central panel with the breadcrumb + table**

In `app.rs`'s `eframe::App::update`, replace the existing central-panel body (the flat `Grid` of rows) with:

```rust
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.entries.is_empty() {
                ui.label("Open an archive to browse its contents.");
                return;
            }

            // Breadcrumb + Up.
            ui.horizontal(|ui| {
                if ui.button("⬆ Up").clicked() && !self.current_dir.is_empty() {
                    self.current_dir = match self.current_dir.rsplit_once('/') {
                        Some((parent, _)) => parent.to_string(),
                        None => String::new(),
                    };
                }
                if ui.link("📁 root").clicked() {
                    self.current_dir.clear();
                }
                let segments: Vec<String> = if self.current_dir.is_empty() {
                    Vec::new()
                } else {
                    self.current_dir.split('/').map(|s| s.to_string()).collect()
                };
                let mut acc = String::new();
                for seg in segments {
                    ui.label("›");
                    if acc.is_empty() { acc = seg.clone(); } else { acc = format!("{acc}/{seg}"); }
                    if ui.link(&seg).clicked() {
                        self.current_dir = acc.clone();
                    }
                }
            });
            ui.separator();

            let nodes = crate::tree::children(&self.entries, &self.current_dir);
            let mut enter_dir: Option<String> = None;

            use egui_extras::{Column, TableBuilder};
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .column(Column::remainder().at_least(180.0)) // Name
                .column(Column::auto().at_least(80.0))        // Size
                .column(Column::auto().at_least(80.0))        // Packed
                .column(Column::auto().at_least(60.0))        // Ratio
                .header(20.0, |mut header| {
                    header.col(|ui| { ui.strong("Name"); });
                    header.col(|ui| { ui.strong("Size"); });
                    header.col(|ui| { ui.strong("Packed"); });
                    header.col(|ui| { ui.strong("Ratio"); });
                })
                .body(|mut body| {
                    for node in &nodes {
                        body.row(20.0, |mut row| {
                            let is_selected =
                                !node.is_dir && self.selected.contains(&node.full_path);
                            row.set_selected(is_selected);

                            row.col(|ui| {
                                let glyph = if node.is_dir { "📁" } else { "📄" };
                                let label = format!("{glyph} {}", node.name);
                                let resp = ui.selectable_label(is_selected, label);
                                if resp.clicked() {
                                    if node.is_dir {
                                        // single click selects; double enters (below)
                                        self.selected.clear();
                                    } else if !self.selected.remove(&node.full_path) {
                                        self.selected.insert(node.full_path.clone());
                                    }
                                }
                                if resp.double_clicked() && node.is_dir {
                                    enter_dir = Some(node.full_path.clone());
                                }
                            });
                            row.col(|ui| {
                                ui.label(if node.is_dir { "—".into() } else { human_size(node.size) });
                            });
                            row.col(|ui| {
                                ui.label(if node.is_dir { "—".into() } else { human_size(node.compressed) });
                            });
                            row.col(|ui| {
                                if node.is_dir || node.size == 0 {
                                    ui.label("—");
                                } else {
                                    let ratio = 100.0 * (1.0 - node.compressed as f64 / node.size as f64);
                                    ui.label(format!("{ratio:.0}%"));
                                }
                            });
                        });
                    }
                });

            if let Some(dir) = enter_dir {
                self.current_dir = dir;
                self.selected.clear();
            }
        });
```

- [ ] **Step 4: Add the `human_size` helper to `app.rs`**

```rust
/// Format a byte count compactly (e.g. 1.5 KB).
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}
```

- [ ] **Step 5: Make "Extract Selected" use the new `selected` set**

If the old extract logic referenced the old per-row `selected` booleans, update `start_extract`'s `selected_only` branch to collect names from `self.selected`:

```rust
        let names: Vec<String> = if selected_only {
            self.selected.iter().cloned().collect()
        } else {
            Vec::new()
        };
```

(Remove the old `rows: Vec<Row>` field and `Row` struct if they are now unused; `cargo clippy` will flag dead code.)

- [ ] **Step 6: Build, smoke, lint**

Run:
```
source "$HOME/.cargo/env"
cargo build --features gui
cargo clippy --features gui --bin rza-gui -- -D warnings
cargo fmt --all -- --check
cargo test --features gui --bin rza-gui
```
Expected: builds; clippy + fmt clean; tree/argv tests still pass.
**egui_extras API caveat:** `TableBuilder`/`Column`/`row.set_selected` are the 0.29 surface; if a method name differs in the resolved version, adapt to the crate's docs and note it in the report.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(gui): navigable file-manager table with folders and columns"
```

---

### Task 5: Toolbar, right-click menu, Test & Info

Add the top toolbar, in-window right-click actions, the Info panel, and wire the integrity Test onto the background job.

**Files:**
- Modify: `src/bin/rza-gui/app.rs`

**Interfaces:**
- Consumes: `archive::test`, the job machinery, `self.selected`/`self.current_dir`, `self.entries`.

- [ ] **Step 1: Add a `start_test` method to `RzaApp`**

Mirror `start_extract`'s threading, calling `archive::test`:

```rust
    fn start_test(&mut self, ctx: &egui::Context) {
        let Some(archive_path) = self.archive_path.clone() else { return; };
        let ctx2 = ctx.clone();
        self.spawn_job(ctx, move |tx| {
            let send = {
                let tx = tx.clone();
                move |p: Progress| { let _ = tx.send(JobMsg::Progress(p)); ctx2.request_repaint(); }
            };
            let result = archive::test(&archive_path, send);
            let _ = tx.send(JobMsg::Done(result.map_err(|e| format!("{e:#}"))));
        });
    }
```

- [ ] **Step 2: Add an `info_open: bool` field + an Info window**

Add `info_open: bool` to `RzaApp` (default `false`). At the end of `update`, render the window when open:

```rust
        if self.info_open {
            let mut open = self.info_open;
            egui::Window::new("Archive info").open(&mut open).show(ctx, |ui| {
                if let Some(p) = &self.archive_path {
                    ui.label(format!("Path: {}", p.display()));
                }
                let files = self.entries.iter().filter(|e| !e.is_dir).count();
                let total: u64 = self.entries.iter().map(|e| e.size).sum();
                let packed: u64 = self.entries.iter().map(|e| e.compressed).sum();
                ui.label(format!("Entries: {files} file(s)"));
                ui.label(format!("Total size: {}", human_size(total)));
                ui.label(format!("Packed size: {}", human_size(packed)));
                if total > 0 {
                    let ratio = 100.0 * (1.0 - packed as f64 / total as f64);
                    ui.label(format!("Overall ratio: {ratio:.0}%"));
                }
            });
            self.info_open = open;
        }
```

- [ ] **Step 3: Replace the top toolbar**

Replace the existing top panel (the ad-hoc Open/New buttons) with a toolbar. The buttons disable while a job runs (`!self.busy()`):

```rust
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_enabled_ui(!self.busy(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("📂 Open").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Archives", &["zip", "tar", "gz", "tgz", "bz2", "xz", "zst"])
                            .pick_file()
                        {
                            self.open_archive(path);
                        }
                    }
                    if ui.button("➕ Add").clicked() {
                        if let Some(files) = rfd::FileDialog::new().pick_files() {
                            self.staged.extend(files);
                            self.status = format!("{} file(s) staged", self.staged.len());
                        }
                    }
                    if ui.button("⬆ Extract").clicked() {
                        self.start_extract(ctx, !self.selected.is_empty());
                    }
                    if ui.button("✓ Test").clicked() {
                        self.start_test(ctx);
                    }
                    if ui.button("ℹ Info").clicked() {
                        self.info_open = true;
                    }
                });
            });
        });
```

(Keep the existing bottom panel with the Create controls/method combo + progress bar + status label. If "Add"/Create lived in the old top panel, move just the Create button + method combo to the bottom panel so the toolbar stays clean.)

- [ ] **Step 4: Add a right-click context menu on file rows**

In the Name column closure from Task 4 (inside `row.col` for the name), attach a context menu to the response:

```rust
                                resp.context_menu(|ui| {
                                    if !node.is_dir {
                                        if ui.button("Extract this…").clicked() {
                                            self.selected.clear();
                                            self.selected.insert(node.full_path.clone());
                                            self.start_extract(ctx, true);
                                            ui.close_menu();
                                        }
                                    }
                                    if ui.button("Extract selected…").clicked() {
                                        self.start_extract(ctx, true);
                                        ui.close_menu();
                                    }
                                    if ui.button("Info").clicked() {
                                        self.info_open = true;
                                        ui.close_menu();
                                    }
                                });
```

**Borrow note:** `start_extract`/`self.*` inside the row closure may conflict with the `&self.entries`/`nodes` borrow. If the borrow checker objects, collect the intended action into a local `enum RowAction { ExtractOne(String), ExtractSelected, Info }` set inside the closure (like `enter_dir`), and perform the `self.*` calls AFTER the table block. Note any such restructuring in the report.

- [ ] **Step 5: Build, smoke, lint**

Run:
```
source "$HOME/.cargo/env"
cargo build --features gui
cargo clippy --features gui --bin rza-gui -- -D warnings
cargo fmt --all -- --check
cargo test --features gui --bin rza-gui
./target/debug/rza-gui &   # optional headless smoke; kill after a few seconds
```
Expected: builds; clippy + fmt clean; tests pass; app launches without panic.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(gui): toolbar, right-click actions, Test and Info"
```

---

## Self-Review Notes

- **Spec coverage:** console fix + module split (Task 1); library `test()` (Task 2); tree model (Task 3); navigation + resizable columns + styling (Task 4); toolbar + in-window right-click + Test + Info (Task 5). Out-of-scope items (in-archive edit, Explorer menu, Modified column, image icons, sorting) are excluded.
- **Placeholder scan:** none — concrete code/commands throughout. The two adaptation notes (egui_extras 0.29 API; right-click borrow restructuring) are explicit fallbacks with instructions, not vague placeholders.
- **Type consistency:** `tree::{Node, children}` defined in Task 3, consumed in Tasks 4–5; `RzaApp.current_dir`/`selected` defined Task 4, used Task 5; `archive::test` defined Task 2, consumed Task 5; `human_size` defined Task 4, reused Task 5; the job machinery (`spawn_job`/`JobMsg`/`Progress`/`busy`) reused unchanged.
- **Known risk (flag to human):** `egui_extras` table API specifics and egui borrow-checker interactions in the row closures may need small adaptations during Tasks 4–5; the plan gives concrete fallbacks, and GUI rendering is verified by build/clippy/smoke (not automated UI tests), consistent with the spec.
