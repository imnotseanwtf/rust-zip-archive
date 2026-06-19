use std::process::Command;

fn rza() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rza"))
}

#[test]
fn cli_creates_and_lists_tar_gz() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let f = root.join("hello.txt");
    std::fs::write(&f, "hi\n").unwrap();
    let archive = root.join("out.tar.gz");

    assert!(rza()
        .arg("create")
        .arg("-o")
        .arg(&archive)
        .arg(&f)
        .status()
        .unwrap()
        .success());
    assert!(archive.exists());

    let out = rza().arg("list").arg(&archive).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("hello.txt"));
}
