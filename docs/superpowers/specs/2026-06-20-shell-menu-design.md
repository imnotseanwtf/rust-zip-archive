# Windows Shell Context Menu (Slice C) — Design Spec

**Date:** 2026-06-20
**Status:** Approved

## Goal

Give `rza` a 7-Zip-style right-click menu in Windows Explorer: a cascading
**rza ▸** submenu offering **Extract Here**, **Extract to "name\\"**, and
**Open with rza** on archive files, and **Add to archive…** on any file/folder.
Plus a cheap Linux "Open with rza" via the `.desktop` `MimeType` fix.

## Approach: registry SubCommands (no COM DLL)

The cascading submenu is built from **Windows registry keys** using the
`SubCommands` mechanism — a parent verb whose `SubCommands` value names child
verbs, each with its own `command`. This reproduces 7-Zip's *concept* (one
"rza ▸" menu with several actions) without a C++/COM `IContextMenu` shell
extension DLL.

**Rejected:** a COM/`IExplorerCommand` shell-extension DLL — the only way to get
submenu icons, dynamic items, and the Windows-11 *native* (short) menu, but a
large, fragile, Windows-only component that cannot be built or tested in this
project's Linux dev/CI-on-mac/win-only-build setup without many blind
iterations. A future upgrade if the polished look is wanted.

**Windows 11 caveat (documented):** registry-based menus appear under
**"Show more options"** on Win11's short menu (same as 7-Zip itself without its
new package), and at top level on Win10. Accepted for this slice.

## 1. Menu contents

Written into the registry by the installer:

- For archive types (`.zip .tar .gz .tgz .bz2 .xz .zst .7z .rar` and the
  `.tar.*` compound extensions as their final extension): a cascading **rza**
  submenu with:
  - **Extract Here** → `rza extract-here "%1"`
  - **Extract to "<name>\\"** → `rza extract-to "%1"`
  - **Open with rza** → `rza-gui "%1"`
- For all files/folders (`*` and `Directory`): **Add to archive…** →
  `rza-gui "%1"` (GUI opens in create mode with the path staged).

The `rza` CLI and `rza-gui` are installed to a known location by the installer;
the registry commands reference their absolute install paths.

## 2. CLI additions

Two shell-friendly subcommands the registry can call with only the file path
(implemented in `src/cli.rs` + `src/bin/rza.rs`, reusing the library):

```
rza extract-here <archive>   # extract into the archive's own directory
rza extract-to   <archive>   # extract into a new sibling folder named after the
                             # archive (its filename minus the recognized
                             # archive/compressor extension)
```

Both resolve the destination from the archive path, then call the existing
`archive::extract(archive, dest, force=false, progress)`. They are thin wrappers
around library logic; the destination-deriving helper is a pure, unit-tested
function:

```rust
// in the library or rza.rs as a pub(crate) helper
fn extract_here_dir(archive: &Path) -> PathBuf;   // archive.parent()
fn extract_to_dir(archive: &Path) -> PathBuf;     // parent/<stem-without-archive-ext>
```

`extract_to_dir` strips a known archive/compressor suffix (`.tar.gz`, `.tgz`,
`.zip`, `.gz`, …) from the file name to form the subfolder name.

## 3. GUI addition — "Add to archive…" / multi-file launch

`rza-gui`'s argument handling is extended:

- If the args are a **single existing archive file** → open it in Browse mode
  (today's behavior, unchanged).
- If the args are **one or more paths that are not a single openable archive**
  (multiple paths, or a directory, or a non-archive file) → start in
  **create mode** with those paths staged (reuse the existing `staged` list and
  create flow).

A pure helper decides this and is unit-tested:

```rust
enum Launch { Open(PathBuf), Stage(Vec<PathBuf>), Empty }
fn launch_intent(args: &[String]) -> Launch;
```

`main` calls `launch_intent` and either `open_archive` or pre-populates `staged`.

## 4. Installer wiring (the uncertain seam)

The registry keys are added by the Windows installer produced by
`cargo-packager`. Whether cargo-packager's NSIS backend supports arbitrary
registry writes directly is uncertain; the expected path is a **custom NSIS
installer hook/template** that issues `WriteRegStr` for the SubCommands keys on
install and removes them on uninstall. The exact keys:

```
HKCU\Software\Classes\<ProgId-or-ext>\shell\rza            (MUI verb "rza", SubCommands="rza.here;rza.to;rza.open")
HKCU\Software\Classes\...\shell\rza\shell\rza.here\command  = "<dir>\rza.exe" extract-here "%1"
HKCU\Software\Classes\...\shell\rza\shell\rza.to\command    = "<dir>\rza.exe" extract-to "%1"
HKCU\Software\Classes\...\shell\rza\shell\rza.open\command  = "<dir>\rza-gui.exe" "%1"
HKCU\Software\Classes\*\shell\rza.add\command               = "<dir>\rza-gui.exe" "%1"
```

Installing under `HKCU\Software\Classes` (per-user) avoids requiring admin and
is removable on uninstall.

## 5. Linux (cheap add)

Add `MimeType=` lines to the generated `.desktop` (the follow-up logged earlier)
so archive types show **Open With → rza** in GNOME Files / Dolphin. Full
"Extract here" file-manager actions are per-DE and out of scope here.

## 6. Error handling

- `extract-here`/`extract-to` on a bad path or unreadable archive print the
  error and exit non-zero (existing CLI error path), so a failed shell action
  surfaces a normal error rather than silently doing nothing.
- `extract_to_dir` on a name with no recognized archive suffix falls back to
  `<name>_extracted` so it never collides with the archive file itself.
- GUI launched with a mix of nonexistent paths ignores the missing ones; if
  none remain it starts empty.

## 7. Testing

- **CLI** (Linux, automated): unit-test `extract_here_dir` / `extract_to_dir`
  (suffix stripping, fallback); integration-test `rza extract-here` and
  `rza extract-to` round-trips (create an archive, run the command, assert files
  land in the expected directory).
- **GUI** (Linux, automated): unit-test `launch_intent` (single archive → Open;
  multiple/dir/non-archive → Stage; nothing → Empty). Startup smoke with various
  args.
- **Shell menu** (Windows, MANUAL by the user): build the installer in CI, then
  install on Windows and verify the rza ▸ submenu and each action; iterate on the
  NSIS registry hook as needed. This cannot be verified in this environment.

## 8. Scope guard (YAGNI)

**In:** Windows registry SubCommands cascading menu (Extract Here / Extract to /
Open / Add to archive…); the supporting CLI subcommands and GUI multi-file
launch; Linux `MimeType` "Open With". **Out:** COM/`IExplorerCommand` DLL,
submenu icons, Windows-11 native (non-"Show more options") menu, macOS Finder
services, quick "Add to name.zip/.7z" presets (only the dialog/GUI "Add to
archive…"), and per-DE Linux "Extract here" actions.
