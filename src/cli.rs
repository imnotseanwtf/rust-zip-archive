use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// rza — a small multi-format archive utility (zip, tar, tar.gz/bz2/xz/zst, gz/bz2/xz/zst).
#[derive(Parser, Debug)]
#[command(name = "rza", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a new archive from files and/or directories.
    #[command(visible_alias = "c")]
    Create {
        /// Path of the archive to create. The format is chosen by the
        /// extension: .zip, .tar, .tar.gz/.tgz, .tar.bz2, .tar.xz, .tar.zst,
        /// or single-file .gz/.bz2/.xz/.zst.
        #[arg(short, long)]
        output: PathBuf,

        /// Files and directories to add to the archive.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// Compression method (applies to .zip only; other formats use the
        /// compression implied by their extension).
        #[arg(short, long, value_enum, default_value_t = Compression::Deflate)]
        method: Compression,

        /// Overwrite the output file if it already exists.
        #[arg(short, long)]
        force: bool,
    },

    /// Extract the contents of an archive.
    #[command(visible_alias = "x")]
    Extract {
        /// Archive to extract.
        archive: PathBuf,

        /// Directory to extract into (defaults to the current directory).
        #[arg(short, long, default_value = ".")]
        dest: PathBuf,

        /// Overwrite existing files when extracting.
        #[arg(short, long)]
        force: bool,
    },

    /// List the contents of an archive.
    #[command(visible_alias = "l")]
    List {
        /// Archive to inspect.
        archive: PathBuf,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Compression {
    /// No compression, just store the files.
    Store,
    /// DEFLATE — the standard, widely compatible ZIP compression.
    Deflate,
    /// BZIP2 — slower but often a smaller archive.
    Bzip2,
    /// Zstandard — fast with a good compression ratio.
    Zstd,
}

impl Compression {
    pub fn to_zip_method(self) -> zip::CompressionMethod {
        match self {
            Compression::Store => zip::CompressionMethod::Stored,
            Compression::Deflate => zip::CompressionMethod::Deflated,
            Compression::Bzip2 => zip::CompressionMethod::Bzip2,
            Compression::Zstd => zip::CompressionMethod::Zstd,
        }
    }
}
