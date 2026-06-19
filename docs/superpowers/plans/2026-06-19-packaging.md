# Installable App Packaging — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Package `rza` as an installable desktop app (Windows/macOS/Linux) with an icon, a launcher entry, and archive file associations that open `rza-gui`.

**Architecture:** `rza-gui` becomes the installed application (with the `rza` CLI shipped alongside). `cargo-packager` produces per-OS installers from `[package.metadata.packager]` config + an `assets/icon.png`. `rza-gui` learns to open a file path passed as its first argument so file associations work. A tagged CI workflow builds the installers on each OS and uploads them to the GitHub Release.

**Tech Stack:** Rust 2021; existing `eframe`/`egui` GUI; `cargo-packager` (build-time tool, not a runtime dep); GitHub Actions.

## Global Constraints

- Edition 2021; targets Linux, macOS, Windows.
- `cargo build` (no features) stays CLI-only; the `gui` feature stays opt-in.
- No new *runtime* crate dependencies (cargo-packager is a dev/build tool).
- Bundle display name "rza — Archive Utility"; identifier `com.imnotseanwtf.rza`.
- File associations cover: `zip, tar, gz, tgz, bz2, xz, zst`.
- Builds are unsigned in v1 (documented).
- macOS double-click-open is a follow-up; argv open-on-launch must work on Windows/Linux.
- The existing raw-binary `release.yml` is kept; packaging is additive.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo clippy --features gui --bin rza-gui -- -D warnings` stay clean.

---

### Task 1: `rza-gui` opens a file passed as an argument

Make the GUI open an archive given as `argv[1]` (the mechanism file associations use), via a small pure, testable helper.

**Files:**
- Modify: `src/bin/rza-gui.rs`

**Interfaces:**
- Produces (in `rza-gui.rs`): `fn first_existing_file(args: &[String]) -> Option<std::path::PathBuf>` — returns the first argument (after the program name) that is an existing file, else `None`.
- Consumes: the existing `RzaApp::default()` and `RzaApp::open_archive(&mut self, path: PathBuf)` (no egui `Context` needed — `open_archive` only calls `archive::list` and sets fields).

- [ ] **Step 1: Add the failing unit tests at the bottom of `src/bin/rza-gui.rs`**

```rust
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
        let only_prog = vec!["rza-gui".to_string()];
        assert_eq!(first_existing_file(&only_prog), None);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `source "$HOME/.cargo/env" && cargo test --features gui --bin rza-gui`
Expected: FAIL to compile — `first_existing_file` not defined.

- [ ] **Step 3: Add the helper and use it in `main`**

Add this free function near the top of `src/bin/rza-gui.rs` (after the `use` lines, before `fn main`):

```rust
/// The first CLI argument (after the program name) that names an existing file.
/// File associations launch the app as `rza-gui <path>`; this is how we find it.
fn first_existing_file(args: &[String]) -> Option<std::path::PathBuf> {
    args.iter()
        .skip(1)
        .map(std::path::PathBuf::from)
        .find(|p| p.is_file())
}
```

Then change `main` to open that file on startup. Replace the existing `main` body's `eframe::run_native(...)` call so the app creator opens the initial archive:

```rust
fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let initial = first_existing_file(&args);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 520.0]),
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
```

(Keep the existing `NativeOptions`/viewport size if it differs — only the closure body that builds `RzaApp` and the `initial` wiring are new. The closure must be `move`.)

- [ ] **Step 4: Run tests + lint**

Run: `source "$HOME/.cargo/env" && cargo test --features gui --bin rza-gui && cargo clippy --features gui --bin rza-gui -- -D warnings && cargo fmt --all -- --check`
Expected: 2 new tests pass; clippy + fmt clean.

- [ ] **Step 5: Confirm CLI-only build is unaffected**

Run: `source "$HOME/.cargo/env" && cargo build`
Expected: builds CLI only (no eframe).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(gui): open an archive passed as a command-line argument"
```

---

### Task 2: App icon + cargo-packager config + local Linux package

Add the source icon and the packaging configuration, install cargo-packager, and prove it produces a Linux package on this machine.

**Files:**
- Create: `assets/icon.png`
- Create: `assets/make_icon.py` (deterministic icon generator, committed for reproducibility)
- Modify: `Cargo.toml` (add `[package.metadata.packager]`)
- Modify: `.gitignore` (ignore the packager output dir)

**Interfaces:**
- Produces: a buildable cargo-packager config; the `rza-gui` (main) + `rza` binaries bundled; file associations for the seven extensions.

- [ ] **Step 1: Create `assets/make_icon.py` (stdlib-only PNG writer)**

```python
#!/usr/bin/env python3
"""Generate assets/icon.png — a 512x512 archive-box icon, no third-party deps."""
import struct, zlib, os

W = H = 512

def pixel(x, y):
    # Rounded-ish blue tile with a lighter centered square (a stylized box).
    border = 40
    inner = 150 <= x < 362 and 150 <= y < 362
    in_tile = border <= x < W - border and border <= y < H - border
    if not in_tile:
        return (0, 0, 0, 0)            # transparent margin
    if inner:
        return (235, 240, 252, 255)    # light face
    return (45, 108, 223, 255)         # blue body

raw = bytearray()
for y in range(H):
    raw.append(0)  # PNG filter type 0 for this scanline
    for x in range(W):
        raw += bytes(pixel(x, y))

def chunk(typ, data):
    body = typ + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
png += chunk(b"IEND", b"")

os.makedirs("assets", exist_ok=True)
with open("assets/icon.png", "wb") as f:
    f.write(png)
print("wrote assets/icon.png", len(png), "bytes")
```

- [ ] **Step 2: Generate the icon**

Run: `python3 assets/make_icon.py`
Expected: prints `wrote assets/icon.png ...`; `file assets/icon.png` reports `PNG image data, 512 x 512`.

- [ ] **Step 3: Install cargo-packager**

Run: `source "$HOME/.cargo/env" && cargo install cargo-packager --locked`
Expected: installs the `cargo-packager` binary. (If already installed, this is a no-op.)

- [ ] **Step 4: Add `[package.metadata.packager]` to `Cargo.toml`**

Append at the end of `Cargo.toml`:

```toml
[package.metadata.packager]
product-name = "rza — Archive Utility"
identifier = "com.imnotseanwtf.rza"
description = "A multi-format archive utility (zip, tar, gz, bz2, xz, zst)."
authors = ["imnotseanwtf"]
icons = ["assets/icon.png"]
before-packaging-command = "cargo build --release --features gui"
binaries = [
  { path = "rza-gui", main = true },
  { path = "rza" },
]

[[package.metadata.packager.file-associations]]
ext = ["zip", "tar", "gz", "tgz", "bz2", "xz", "zst"]
description = "Archive"
```

**Crate/tool caveat (read before continuing):** the exact key names and shapes
of cargo-packager's config evolve between versions. Run `cargo packager --help`
and consult the installed version's docs/JSON schema, and ADAPT the keys above
to what the installed version accepts (e.g. `product-name` vs `product_name`,
the `file-associations` field name, the `binaries`/`main` shape). The
acceptance test is Step 5 producing a `.deb`. Record any key changes you made in
your report.

- [ ] **Step 5: Build a Linux package to verify the config**

Run:
```
source "$HOME/.cargo/env"
cargo build --release --features gui
cargo packager --release --formats deb
```
Expected: a `.deb` is produced (default output under `target/release` or
`dist/`). Confirm with: `find target dist -name '*.deb' 2>/dev/null`.
If the environment is missing a system tool required only for `.deb`/AppImage
(report the exact error), treat local artifact generation as deferred to CI, but
the config MUST be accepted by `cargo packager` (no config-parse error) — that
is the minimum bar for this task. Note the outcome in your report.

- [ ] **Step 6: Ignore the packager output**

Add to `.gitignore`:

```
/dist
```

- [ ] **Step 7: Commit**

```bash
git add assets/make_icon.py assets/icon.png Cargo.toml .gitignore
git commit -m "feat: app icon and cargo-packager configuration"
```

---

### Task 3: CI workflow to build installers on all three OSes

Add a tag-triggered workflow that packages on Linux/macOS/Windows and uploads installers to the GitHub Release.

**Files:**
- Create: `.github/workflows/package.yml`
- Modify: `.github/workflows/ci.yml` (run the GUI bin's unit tests so `first_existing_file` is covered in CI)

- [ ] **Step 1: Add GUI bin tests to the CI `gui` job in `.github/workflows/ci.yml`**

In the existing `gui` job, after the `Build GUI` step, add a test step:

```yaml
      - name: Test GUI bin
        run: cargo test --features gui --bin rza-gui
```

- [ ] **Step 2: Create `.github/workflows/package.yml`**

```yaml
name: Package

# Build installable app bundles and attach them to a GitHub Release when a
# version tag (e.g. v0.1.0) is pushed.
on:
  push:
    tags: ["v*"]

permissions:
  contents: write

env:
  CARGO_TERM_COLOR: always

jobs:
  package:
    name: Package (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Install Linux GUI + packaging deps
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y libxkbcommon-dev libwayland-dev libxcb1-dev \
            libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libgl1-mesa-dev \
            libgtk-3-dev

      - name: Install cargo-packager
        run: cargo install cargo-packager --locked

      - name: Build GUI binary
        run: cargo build --release --features gui

      - name: Package
        run: cargo packager --release

      - name: Upload installers to release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            dist/*
            target/release/*.deb
            target/release/*.AppImage
            target/release/*.dmg
            target/release/*.msi
            target/release/*.exe
          fail_on_unmatched_files: false
```

**Note:** `cargo packager`'s default output location varies by version (`dist/`
or alongside `target/release`). The upload globs cover both; `fail_on_unmatched_files:
false` keeps the job green when a given OS only produces some artifact types.

- [ ] **Step 3: Validate the workflow YAML**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/package.yml')); yaml.safe_load(open('.github/workflows/ci.yml')); print('OK')"`
Expected: `OK`.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/package.yml .github/workflows/ci.yml
git commit -m "ci: build installable app bundles on tag for all three OSes"
```

---

### Task 4: README — "Install the app" section

Document installing the app and the one-time unsigned-app steps.

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add an "Install the app" section near the top of `README.md`**

Insert after the intro paragraph (before "## Supported formats"):

```markdown
## Install the app

Download the installer for your OS from the
[Releases page](https://github.com/imnotseanwtf/rust-zip-archive/releases):

- **Windows:** run the `.msi` / `.exe` installer → launch **rza — Archive
  Utility** from the Start menu.
- **macOS:** open the `.dmg` and drag the app to Applications. The build is
  unsigned, so the first launch needs **right-click → Open** once (Gatekeeper).
- **Linux:** install the `.deb` (`sudo apt install ./rza_*.deb`) or run the
  `.AppImage` (`chmod +x rza_*.AppImage && ./rza_*.AppImage`).

The installer registers `rza` (the CLI) and `rza-gui` (the window) and
associates archive types (`.zip`, `.tar`, `.tar.gz`, `.tar.xz`, `.tar.zst`,
`.gz`, `.bz2`, `.xz`, `.zst`) so you can **open an archive with the app**.

> Windows shows a SmartScreen warning for unsigned installers — choose
> **More info → Run anyway**. On macOS use right-click → Open the first time.
> (Double-click-to-open an archive works on Windows/Linux; on macOS launch the
> app or use `open -a "rza — Archive Utility" file.zip` for now.)
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: how to install the app and handle unsigned builds"
```

---

## Self-Review Notes

- **Spec coverage:** open-on-launch via argv + testable helper (Task 1); icon asset + cargo-packager config with binaries & file associations + local Linux build (Task 2); 3-OS CI packaging workflow + GUI-bin tests in CI (Task 3); README install + unsigned-app docs (Task 4). Kept `release.yml` untouched (additive packaging). Unsigned builds documented. macOS double-click-open explicitly deferred (README + spec).
- **Placeholder scan:** none — every step has concrete code/commands.
- **Type consistency:** `first_existing_file(&[String]) -> Option<PathBuf>` defined and used in Task 1; `open_archive` reused as the existing method; identifier/name/extensions match the spec verbatim across Tasks 2–4.
- **Known risk (flagged to the human):** cargo-packager's config schema and output paths vary by version — Task 2 instructs the implementer to adapt keys to the installed version and report changes; full Windows/macOS installer verification happens in CI (Task 3), not on the Linux dev machine. If `cargo packager` cannot produce a `.deb` locally due to a missing system tool, the task bar is "config accepted without parse error," with artifact generation deferred to CI.
