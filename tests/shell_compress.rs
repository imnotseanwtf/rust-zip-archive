use std::process::Command;

fn rza() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rza"))
}

#[test]
fn compress_zip_creates_named_archive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let folder = root.join("photos");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "a").unwrap();

    assert!(rza()
        .arg("compress-zip")
        .arg(&folder)
        .status()
        .unwrap()
        .success());
    let archive = root.join("photos.zip");
    assert!(
        archive.exists(),
        "photos.zip should be created next to the folder"
    );

    let out = rza().arg("list").arg(&archive).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("a.txt"));
}

#[test]
fn compress_targz_creates_named_archive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let folder = root.join("docs");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("b.txt"), "b").unwrap();

    assert!(rza()
        .arg("compress-targz")
        .arg(&folder)
        .status()
        .unwrap()
        .success());
    assert!(
        root.join("docs.tar.gz").exists(),
        "docs.tar.gz should be created"
    );
}

#[test]
fn test_command_passes_and_fails() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let f = root.join("data.txt");
    std::fs::write(&f, "hello\n".repeat(50)).unwrap();
    let archive = root.join("good.zip");
    assert!(rza()
        .arg("create")
        .arg("-o")
        .arg(&archive)
        .arg(&f)
        .status()
        .unwrap()
        .success());

    // Good archive -> exit 0.
    assert!(rza().arg("test").arg(&archive).status().unwrap().success());

    // Corrupt it (truncate) -> non-zero exit.
    let data = std::fs::read(&archive).unwrap();
    std::fs::write(&archive, &data[..data.len() / 2]).unwrap();
    assert!(!rza().arg("test").arg(&archive).status().unwrap().success());
}
