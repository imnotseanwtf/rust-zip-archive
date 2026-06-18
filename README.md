# rust-zip-archive (`rza`)

[![CI](https://github.com/imnotseanwtf/rust-zip-archive/actions/workflows/ci.yml/badge.svg)](https://github.com/imnotseanwtf/rust-zip-archive/actions/workflows/ci.yml)

A small 7-Zip/WinRAR-style command-line archive utility written in Rust.
Runs on **Linux, macOS, and Windows** — every push is built and tested on all three.

## Features (v1)

- **Create** `.zip` archives from files and directories
- **Extract** archives (with safe path handling — no zip-slip)
- **List** archive contents with sizes and compression ratios
- Selectable compression: `store`, `deflate`, `bzip2`, `zstd`
- Progress bars for create/extract
- Streams files instead of buffering them in memory (safe for large files)
- Preserves Unix file permissions (executable bit) on Linux/macOS
- Windows-safe extraction: reserved device names (`CON`, `NUL`, …) and illegal
  filename characters are rewritten automatically

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

- `.tar`, `.tar.gz`, `.tar.zst` support via the `tar` + `flate2`/`zstd` crates
- `xz` support via `xz2`
- Password-protected (AES) zips
- A TUI or GUI front end
