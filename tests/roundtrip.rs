//! End-to-end tests that drive the compiled `rza` binary, so they exercise the
//! same code path users do. These run on Linux, macOS, and Windows in CI.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Path to the binary built by Cargo for this integration test.
fn rza() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rza"))
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

#[test]
fn create_list_extract_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Build a small tree to archive.
    let src = root.join("sample");
    write(&src.join("notes.txt"), &"hello world\n".repeat(100));
    write(&src.join("nested/deep.txt"), "deep file\n");

    let archive = root.join("backup.zip");

    // create
    let status = rza()
        .arg("create")
        .arg("-o")
        .arg(&archive)
        .arg(&src)
        .status()
        .unwrap();
    assert!(status.success(), "create failed");
    assert!(archive.exists(), "archive was not written");

    // list
    let out = rza().arg("list").arg(&archive).output().unwrap();
    assert!(out.status.success(), "list failed");
    let listing = String::from_utf8_lossy(&out.stdout);
    assert!(listing.contains("notes.txt"), "listing missing notes.txt");
    assert!(listing.contains("deep.txt"), "listing missing deep.txt");

    // extract
    let dest = root.join("restored");
    let status = rza()
        .arg("extract")
        .arg(&archive)
        .arg("--dest")
        .arg(&dest)
        .status()
        .unwrap();
    assert!(status.success(), "extract failed");

    // round-trip: contents must match
    let original = fs::read(src.join("notes.txt")).unwrap();
    let restored = fs::read(dest.join("sample/notes.txt")).unwrap();
    assert_eq!(original, restored, "notes.txt content changed");

    let deep = fs::read_to_string(dest.join("sample/nested/deep.txt")).unwrap();
    assert_eq!(deep, "deep file\n", "nested file content changed");
}

#[test]
fn create_into_missing_output_dir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let src = root.join("data.txt");
    write(&src, "some data\n");

    // Output dir "out" does not exist yet; create should make it.
    let archive = root.join("out/nested/backup.zip");
    let status = rza()
        .arg("create")
        .arg("-o")
        .arg(&archive)
        .arg(&src)
        .status()
        .unwrap();
    assert!(status.success(), "create into missing dir failed");
    assert!(archive.exists(), "archive not created in new directory");
}

#[cfg(unix)]
#[test]
fn preserves_executable_bit() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let script = root.join("run.sh");
    write(&script, "#!/bin/sh\necho hi\n");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let archive = root.join("backup.zip");
    assert!(rza()
        .arg("create")
        .arg("-o")
        .arg(&archive)
        .arg(&script)
        .status()
        .unwrap()
        .success());

    let dest = root.join("out");
    assert!(rza()
        .arg("extract")
        .arg(&archive)
        .arg("--dest")
        .arg(&dest)
        .status()
        .unwrap()
        .success());

    let mode = fs::metadata(dest.join("run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert!(mode & 0o111 != 0, "executable bit was not preserved");
}

#[test]
fn list_returns_entry_metadata() {
    use rust_zip_archive::archive::{self, EntryInfo};
    use rust_zip_archive::cli::Compression;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = root.join("data.txt");
    write(&src, &"x".repeat(500));

    let archive = root.join("a.zip");
    archive::create(
        &archive,
        std::slice::from_ref(&src),
        Compression::Deflate,
        false,
        |_p| {},
    )
    .unwrap();

    let entries: Vec<EntryInfo> = archive::list(&archive).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "data.txt");
    assert_eq!(entries[0].size, 500);
    assert!(entries[0].compressed > 0);
    assert!(!entries[0].is_dir);
}
