# rza GUI — Design Spec

**Date:** 2026-06-19
**Status:** Approved (pending spec review)

## Goal

Add a native, cross-platform graphical front end to `rza` that feels like
7-Zip/WinRAR: a resizable desktop window for **creating**, **opening/browsing**,
and **extracting** `.zip` archives. Runs on Linux, macOS, and Windows.

Chosen toolkit: **egui / eframe** (pure Rust, single native binary per OS,
reuses the existing archive logic).

## Interactions (v1)

- Drag-and-drop files/folders into the window to add them to a new archive.
- Selective extraction via per-entry checkboxes.
- Native file/folder dialogs (via the `rfd` crate).
- A live progress bar in the window during create/extract.

## 1. Architecture: shared library + two binaries

Refactor the current binary-only crate into a library plus two thin binaries so
the CLI and GUI share one source of truth.

```
src/lib.rs         → public API: create(), extract(), extract_selected(), list()
src/archive.rs     → core logic, re-exported by lib.rs
src/cli.rs         → CLI arg parsing (unchanged)
src/bin/rza.rs     → CLI binary (today's main.rs), calls the lib
src/bin/rza-gui.rs → NEW egui window, calls the same lib
```

GUI dependencies (`eframe`/`egui`, `rfd`) are gated behind a **`gui` Cargo
feature**:

- `cargo build` → CLI only (no graphics stack compiled).
- `cargo build --features gui` → builds the `rza-gui` binary too.
- The `[[bin]]` entry for `rza-gui` uses `required-features = ["gui"]`.

**Rationale:** keeps the CLI lean, gives both front ends one implementation of
the archive logic, and keeps each unit independently testable.

## 2. Library API changes

Two changes to the core functions, needed so the logic is UI-agnostic:

1. **Progress via callback.** Today `create`/`extract` build `indicatif` bars
   internally. Change them to accept a progress callback:

   ```rust
   pub struct Progress { pub current: u64, pub total: u64, pub message: String }
   ```

   Functions take `progress: impl FnMut(Progress)`. The CLI wires this to
   `indicatif` (identical terminal behavior to today). The GUI wires it to its
   progress state.

2. **Selective extraction.** Add:

   ```rust
   pub fn extract_selected(archive: &Path, dest: &Path, names: &[String],
                           force: bool, progress: impl FnMut(Progress)) -> Result<()>;
   ```

   The existing "extract everything" becomes a thin wrapper that passes all
   entry names.

`list()` returns a `Vec` of entry metadata (name, size, compressed size, ratio,
is_dir) for the GUI table and the CLI listing.

## 3. The window (egui)

A single window with two modes — **Browse** (an archive is open) and **Create**
(building a new archive).

```
┌─ rza — Archive Utility ─────────────────────────────────┐
│ [ Open Archive… ]  [ New Archive ]          Mode: Browse │
├─────────────────────────────────────────────────────────┤
│  Drop files here to add them, or use the buttons above   │
│                                                          │
│  ☑  Name                  Size      Compressed   Ratio   │
│  ──────────────────────────────────────────────────     │
│  ☑  src/main.rs           1.2 KB    640 B        47%     │
│  ☑  src/archive.rs        7.0 KB    2.1 KB       70%     │
│  ☐  README.md             1.5 KB    800 B        47%     │
├─────────────────────────────────────────────────────────┤
│ Method: [Deflate ▾]  [Extract Selected…] [Extract All…]  │
│ ▓▓▓▓▓▓▓▓▓░░░░░░ 62%  Extracting src/archive.rs           │
└─────────────────────────────────────────────────────────┘
```

- **Open Archive…** → `rfd` file dialog → `list()` populates the table (Browse).
- **New Archive** or **drag-and-drop** → Create mode: a list of staged input
  paths, a compression-method dropdown, and a **Create…** button that picks an
  output path and writes the archive.
- **Checkboxes** drive **Extract Selected**; **Extract All** ignores them. Both
  prompt for a destination folder via `rfd`.
- **Progress bar** at the bottom updates live during any job.

## 4. Threading & data flow

egui repaints on the UI thread, so create/extract run on a **background
`std::thread`**:

- The worker sends `Progress` messages over an `mpsc` channel.
- Each frame, the UI drains the channel, updates the bar, and calls
  `ctx.request_repaint()`.
- Action buttons are disabled while a job runs.
- On completion the worker sends `Done(Result<()>)`; the UI shows success or an
  error message.

## 5. Error handling

- The lib returns `anyhow::Result`; the GUI converts errors into a dismissible
  message banner instead of panicking (bad zip, permission denied, etc.).
- Unsafe archive paths (zip-slip) and Windows-reserved names are already handled
  in the existing extract logic and remain in force.

## 6. Testing

- Existing integration tests are repointed at the **library API** (rather than
  shelling out to the binary) and extended to cover `extract_selected`.
- The egui rendering layer is not unit-tested (standard for egui apps), but all
  logic it invokes is covered by the library tests.

## 7. CI

- The current 3-OS matrix keeps building + testing the CLI.
- Add a step that builds `cargo build --features gui` on each OS. On the Ubuntu
  runner, install the system libraries egui/winit need (e.g.
  `libxkbcommon-dev`, `libwayland-dev`, `libxcb*`, GL libs) before that step.

## 8. Scope guard (YAGNI)

v1 is one window doing create / open / extract with the four interactions above.
Explicitly **out of scope** for v1:

- Editing an existing archive in place (adding/removing entries).
- Browsing nested archives.
- A persistent settings/preferences panel.
- Formats other than `.zip` (tar/gz/xz remain CLI roadmap items).
