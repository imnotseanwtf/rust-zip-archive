use rust_zip_archive::archive;
use rust_zip_archive::cli::Compression;
use std::fs;
use std::path::Path;

fn write(path: &Path, contents: &str) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn good_archive(ext: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("sample");
    write(&src.join("a.txt"), &"data\n".repeat(100));
    let archive = dir.path().join(format!("a{ext}"));
    archive::create(
        &archive,
        std::slice::from_ref(&src),
        Compression::Deflate,
        false,
        |_p| {},
    )
    .unwrap();
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
            archive::create(
                &a,
                std::slice::from_ref(&f),
                Compression::Deflate,
                false,
                |_p| {},
            )
            .unwrap();
            (dir, a)
        } else {
            good_archive(ext)
        };
        archive::test(&archive, |_p| {}).unwrap_or_else(|e| panic!("{ext} should pass: {e}"));
    }
}

#[test]
fn test_fails_on_corrupt_archive() {
    let (_d, archive) = good_archive(".zip");
    // Truncate the file to corrupt it.
    let data = fs::read(&archive).unwrap();
    fs::write(&archive, &data[..data.len() / 2]).unwrap();
    assert!(
        archive::test(&archive, |_p| {}).is_err(),
        "truncated zip should fail test"
    );
}
