# rust-zip-archive (`rza`)

[![CI](https://github.com/imnotseanwtf/rust-zip-archive/actions/workflows/ci.yml/badge.svg)](https://github.com/imnotseanwtf/rust-zip-archive/actions/workflows/ci.yml)

A small multi-format command-line archive utility written in Rust.
Runs on **Linux, macOS, and Windows** — every push is built and tested on all three.

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

## Features (v1)

- **Create**, **extract**, and **list** archives in multiple formats
- Safe path handling — no zip-slip
- Selectable compression for `.zip`: `store`, `deflate`, `bzip2`, `zstd`
- Progress bars for create/extract
- Streams files instead of buffering them in memory (safe for large files)
- Preserves Unix file permissions (executable bit) on Linux/macOS
- Windows-safe extraction: reserved device names (`CON`, `NUL`, …) and illegal
  filename characters are rewritten automatically

## Supported formats

| Format | List | Extract | Create |
|--------|------|---------|--------|
| `.zip` | yes | yes | yes |
| `.tar`, `.tar.gz`/`.tgz`, `.tar.bz2`, `.tar.xz`, `.tar.zst` | yes | yes | yes |
| `.gz`, `.bz2`, `.xz`, `.zst` (single file) | yes | yes | yes |

The format is auto-detected on extract/list (by content), and chosen from the
output extension on create. `--method` applies to `.zip` only.

## Install

### Prebuilt binaries (no Rust needed)

Grab the archive for your platform from the
[Releases page](https://github.com/imnotseanwtf/rust-zip-archive/releases),
unpack it, and put `rza` (or `rza.exe`) somewhere on your `PATH`.

### Build from source

First install the Rust toolchain:

- **Linux / macOS:**
  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
  (on macOS you can also use `brew install rustup` then `rustup-init`)
- **Windows:** download and run [`rustup-init.exe`](https://rustup.rs), or
  ```powershell
  winget install Rustlang.Rustup
  ```

Then build:

```sh
cargo build --release
# binary at target/release/rza        (Linux/macOS)
# binary at target/release/rza.exe    (Windows)

# or install it onto your PATH:
cargo install --path .
```

## Usage

```sh
# Create an archive
rza create -o backup.zip src/ notes.txt

# Pick a compression method
rza create -o backup.zip --method zstd src/

# List contents
rza list backup.zip

# Extract into a directory
rza extract backup.zip --dest ./restored

# Short aliases also work: c / x / l
rza c -o backup.zip src/
```

## GUI

`rza` also ships an optional native desktop GUI (egui), built behind the `gui`
feature so the CLI stays lightweight.

```sh
# build & run the GUI
cargo run --features gui --bin rza-gui

# or build it
cargo build --release --features gui   # target/release/rza-gui[.exe]
```

In the window you can:
- **Open Archive…** to browse a `.zip`'s contents and **Extract All** or
  **Extract Selected** (tick the boxes).
- **New Archive…** or **drag-and-drop** files in, pick a compression method,
  and **Create…** a new `.zip`.

On Linux, building the GUI needs a few system libraries:

```sh
sudo apt-get install -y libxkbcommon-dev libwayland-dev libxcb1-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libgl1-mesa-dev
```

## Releasing

Prebuilt binaries for Linux, macOS (Intel + Apple Silicon), and Windows are
built automatically when a version tag is pushed:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The `Release` workflow compiles each target and attaches the archives to a new
GitHub Release.

## Roadmap ideas

- Password-protected (AES) zips
