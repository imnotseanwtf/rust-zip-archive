# GUI Redesign (7-Zip-style) + Windows Console Fix — Design Spec

**Date:** 2026-06-20
**Status:** Approved (pending spec review)

## Goal

Turn `rza-gui` from a flat checkbox list into a 7-Zip-style file-manager window:
navigate folders inside an archive, a toolbar, real resizable columns, in-window
right-click actions, and an integrity "Test". Also fix the Windows bug where a
`cmd` console window appears behind the GUI.

This is two of three related slices the user asked for:
- **Slice A (here):** Windows console-window fix.
- **Slice B (here):** the GUI redesign.
- **Slice C (future, separate spec):** Explorer/Finder right-click "Extract
  here" shell integration.

## Slice A — Windows console fix

A Rust binary defaults to the console subsystem, so launching the windowed app
on Windows also opens a `cmd` window. Fix: add, at the top of the GUI binary's
entry point,

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```

Guarded so it only applies to release builds (debug keeps the console for
developer logging). No console window appears for installed (release) users.

## Slice B — GUI redesign

### 1. Restructure the GUI binary into modules

`src/bin/rza-gui.rs` becomes a directory binary `src/bin/rza-gui/` with:
- `main.rs` — entry point (`windows_subsystem` attr, arg parsing, `run_native`).
- `app.rs` — `RzaApp` state + the `eframe::App` view/update logic.
- `tree.rs` — the archive folder-tree model (pure, unit-testable).

Cargo resolves `src/bin/rza-gui/main.rs` as the `rza-gui` binary automatically;
the `[[bin]]` path in `Cargo.toml` changes to `src/bin/rza-gui/main.rs`.

### 2. Archive tree model (`tree.rs`)

The library's `list()` returns a flat `Vec<EntryInfo>` with paths like
`src/nested/c.txt`. `tree.rs` builds a navigable model:

```rust
pub struct Node {
    pub name: String,        // this component's display name
    pub full_path: String,   // full archive path (for files; "" for synthesized dirs)
    pub is_dir: bool,
    pub size: u64,
    pub compressed: u64,
}

/// Immediate children of `dir` (a slash path, "" = root), folders first then
/// files, each sorted by name. Synthesizes intermediate dirs that aren't
/// explicit entries in the archive.
pub fn children(entries: &[EntryInfo], dir: &str) -> Vec<Node>;
```

`children` is pure (no egui), so it is unit-tested directly: e.g. given entries
`["a/b.txt", "a/c/d.txt", "top.txt"]`, `children(.., "")` → `[dir "a", file
"top.txt"]`; `children(.., "a")` → `[dir "a/c", file "a/b.txt"]`.

### 3. App state & navigation (`app.rs`)

- `entries: Vec<EntryInfo>` (from `open_archive`), `current_dir: String`
  (slash path within the archive, "" = root), `selected: HashSet<String>`
  (full paths of selected files).
- Double-click a folder row → `current_dir = node.full_path`. **Up** button /
  breadcrumb segment → set `current_dir` to the parent / clicked segment.
- Opening a new archive resets `current_dir` to "" and clears selection.

### 4. Layout

```
┌─ rza — Archive Utility ─────────────────────────────────────┐
│ [📂 Open] [➕ Add] [⬆ Extract] [✓ Test] [ℹ Info]            │  toolbar
├─────────────────────────────────────────────────────────────┤
│ 📁 backup.zip  ›  src  ›  nested            [⬆ Up]           │  breadcrumb
├─────────────────────────────────────────────────────────────┤
│  Name                    Size      Packed     Ratio          │  resizable cols
│  📁 assets                  —         —          —            │
│  📄 notes.txt           11.0 KB    109 B       99%           │
├─────────────────────────────────────────────────────────────┤
│ ▓▓▓▓▓▓░░░░ 60%  Extracting…        4 items, 12.5 KB          │  status/progress
└─────────────────────────────────────────────────────────────┘
```

- **Toolbar** buttons (glyph + label): Open, Add, Extract, Test, Info.
- **Breadcrumb** shows the archive name + current path; segments and an **Up**
  button navigate.
- **Table** via `egui_extras::TableBuilder`: resizable columns Name (folder/file
  glyph + name), Size, Packed (compressed), Ratio. Folders show `—` for sizes.
  Row click toggles selection (highlight); double-click a folder enters it.
- **Right-click a row** → in-window context menu: *Extract this…*, *Extract
  selected…*, *Info*.
- **Status bar**: progress bar during jobs + a summary (item count / total size).

### 5. Toolbar actions

- **Open** — file dialog → `archive::list` → populate `entries`, reset nav.
- **Add** — the existing create flow: stage files (picker or drag-drop), choose a
  method, **Create…** a new archive (a small dialog/section; create logic
  unchanged).
- **Extract** — Extract All (or Extract Selected when rows are selected) to a
  chosen folder, on the existing background-job machinery.
- **Test** — integrity check (see §6); result shown in the status bar / a banner.
- **Info** — a panel/window: archive path, format, entry count, total
  size/compressed, overall ratio.

### 6. Library: `test` (integrity)

Add to the archive library:

```rust
pub fn test(archive: &Path, progress: impl FnMut(Progress)) -> Result<()>;
```

It streams every entry through `std::io::sink()` (for zip, this validates CRCs;
for the others it validates that decompression succeeds end-to-end), reporting
progress. Returns `Ok(())` if all entries read cleanly, else the first error.
Dispatched per-format like the other operations (single-file compressors:
decompress fully to sink).

### 7. Dependencies

Add `egui_extras = { version = "0.29", default-features = false }` (for
`TableBuilder`) under the `gui` feature's optional deps. No image-loading crate
(glyphs cover icons).

### 8. Reuse / unchanged

The background-job channel (`spawn_job`/`JobMsg`/`poll_job`), `Progress`/
`EntryInfo`, `archive::{create, extract, extract_selected, list}`, drag-and-drop,
and the argv "open on launch" all stay. The redesign is the view layer + tree
model + the new `test` function.

## Error handling

- Opening a non-archive / corrupt file shows the existing error in the status bar
  (no crash).
- `test` failure shows the first error message in the status bar / banner; it
  never panics the UI thread (runs on the background job like extract).
- Navigation into a synthesized folder with no entries shows an empty list, never
  an error.

## Testing

- **`tree::children`** — pure unit tests for nesting, folder-before-file
  ordering, synthesized intermediate dirs, and root vs sub-dir.
- **`test()`** — library integration tests: a good archive returns `Ok`; a
  deliberately truncated/corrupted archive returns `Err`. Cover zip + one
  tarball + one single-file compressor.
- **GUI view** — not unit-tested (standard for egui); a headless startup smoke
  test on Linux confirms it launches with and without a file argument.
- Lint: `cargo clippy --features gui --bin rza-gui -- -D warnings` and
  `cargo fmt --all -- --check` stay clean.

## Scope guard (YAGNI)

**In:** Windows console fix; GUI module restructure; folder navigation; toolbar;
resizable columns + styling; in-window right-click (Extract this / Extract
selected / Info); integrity `test`. **Out:** in-archive editing (delete/rename/
add-to-existing — separate slice); Explorer/Finder shell context menu (Slice C);
a "Modified" column (library exposes no mtime yet); image-based custom icons
(glyphs only); column sorting.
