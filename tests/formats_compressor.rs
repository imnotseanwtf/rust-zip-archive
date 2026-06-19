use rust_zip_archive::archive;
use rust_zip_archive::cli::Compression;
use std::fs;
use std::path::Path;

fn write(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

fn round_trip(ext: &str) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let input = root.join("data.txt");
    let body = "compress me\n".repeat(100);
    write(&input, &body);

    let archive = root.join(format!("data.txt{ext}"));
    archive::create(
        &archive,
        std::slice::from_ref(&input),
        Compression::Deflate,
        false,
        |_p| {},
    )
    .unwrap();
    assert!(archive.exists(), "{ext}: not created");

    let entries = archive::list(&archive).unwrap();
    assert_eq!(entries.len(), 1, "{ext}: should list exactly one entry");
    assert_eq!(entries[0].name, "data.txt", "{ext}: inner name");

    let dest = root.join("out");
    archive::extract(&archive, &dest, false, |_p| {}).unwrap();
    let restored = fs::read_to_string(dest.join("data.txt")).unwrap();
    assert_eq!(restored, body, "{ext}: content mismatch");
}

#[test]
fn gz_round_trip() {
    round_trip(".gz");
}
#[test]
fn bz2_round_trip() {
    round_trip(".bz2");
}
#[test]
fn xz_round_trip() {
    round_trip(".xz");
}
#[test]
fn zst_round_trip() {
    round_trip(".zst");
}

#[test]
fn gz_rejects_multiple_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let a = root.join("a.txt");
    write(&a, "a");
    let b = root.join("b.txt");
    write(&b, "b");
    let archive = root.join("out.gz");
    let err = archive::create(&archive, &[a, b], Compression::Deflate, false, |_p| {}).unwrap_err();
    assert!(
        err.to_string().contains("single file"),
        "expected single-file error, got: {err}"
    );
}
