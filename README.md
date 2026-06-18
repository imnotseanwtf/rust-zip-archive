# rust-zip-archive (`rza`)

A small 7-Zip/WinRAR-style command-line archive utility written in Rust.

## Features (v1)

- **Create** `.zip` archives from files and directories
- **Extract** archives (with safe path handling — no zip-slip)
- **List** archive contents with sizes and compression ratios
- Selectable compression: `store`, `deflate`, `bzip2`, `zstd`
- Progress bars for create/extract
- Streams files instead of buffering them in memory (safe for large files)
- Preserves Unix file permissions, including the executable bit

## Install Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Build

```sh
cargo build --release
# binary at target/release/rza
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

## Roadmap ideas

- `.tar`, `.tar.gz`, `.tar.zst` support via the `tar` + `flate2`/`zstd` crates
- `xz` support via `xz2`
- Password-protected (AES) zips
- A TUI or GUI front end
