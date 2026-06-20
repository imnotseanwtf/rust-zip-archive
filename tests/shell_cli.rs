use std::process::Command;

fn rza() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rza"))
}

#[test]
fn extract_here_lands_in_archive_dir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = root.join("data.txt");
    std::fs::write(&src, "hello\n").unwrap();
    let archive = root.join("bundle.zip");
    assert!(rza()
        .arg("create")
        .arg("-o")
        .arg(&archive)
        .arg(&src)
        .status()
        .unwrap()
        .success());

    // Remove the original so extract-here must recreate it.
    std::fs::remove_file(&src).unwrap();
    assert!(!root.join("data.txt").exists());

    assert!(rza()
        .arg("extract-here")
        .arg(&archive)
        .status()
        .unwrap()
        .success());
    // "Extract Here" puts entries directly in the archive's folder.
    assert!(root.join("data.txt").exists());
}

#[test]
fn extract_to_lands_in_named_subfolder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = root.join("data.txt");
    std::fs::write(&src, "hello\n").unwrap();
    let archive = root.join("bundle.zip");
    assert!(rza()
        .arg("create")
        .arg("-o")
        .arg(&archive)
        .arg(&src)
        .status()
        .unwrap()
        .success());

    assert!(rza()
        .arg("extract-to")
        .arg(&archive)
        .status()
        .unwrap()
        .success());
    // "Extract to bundle\" puts entries under a subfolder named after the archive.
    assert!(root.join("bundle").join("data.txt").exists());
}
