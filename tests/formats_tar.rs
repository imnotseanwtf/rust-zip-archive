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

fn round_trip(ext: &str) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = root.join("sample");
    write(&src.join("a.txt"), &"hello\n".repeat(50));
    write(&src.join("nested/b.txt"), "deep\n");

    let archive = root.join(format!("out{ext}"));
    archive::create(
        &archive,
        std::slice::from_ref(&src),
        Compression::Deflate,
        false,
        |_p| {},
    )
    .unwrap();
    assert!(archive.exists(), "{ext}: not created");

    let entries = archive::list(&archive).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(
        names.iter().any(|n| n.ends_with("a.txt")),
        "{ext}: a.txt missing in list"
    );

    let dest = root.join("out");
    archive::extract(&archive, &dest, false, |_p| {}).unwrap();
    let restored = fs::read_to_string(dest.join("sample/a.txt")).unwrap();
    assert_eq!(restored, "hello\n".repeat(50), "{ext}: content mismatch");
}

#[test]
fn tar_round_trip() {
    round_trip(".tar");
}
#[test]
fn tar_gz_round_trip() {
    round_trip(".tar.gz");
}
#[test]
fn tar_bz2_round_trip() {
    round_trip(".tar.bz2");
}
#[test]
fn tar_xz_round_trip() {
    round_trip(".tar.xz");
}
#[test]
fn tar_zst_round_trip() {
    round_trip(".tar.zst");
}

#[test]
fn tar_extract_selected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = root.join("s");
    write(&src.join("keep.txt"), "k");
    write(&src.join("skip.txt"), "s");
    let archive = root.join("a.tar");
    archive::create(
        &archive,
        std::slice::from_ref(&src),
        Compression::Deflate,
        false,
        |_p| {},
    )
    .unwrap();
    let dest = root.join("out");
    archive::extract_selected(&archive, &dest, &["s/keep.txt".into()], false, |_p| {}).unwrap();
    assert!(dest.join("s/keep.txt").exists());
    assert!(!dest.join("s/skip.txt").exists());
}
