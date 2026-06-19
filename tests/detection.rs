use rust_zip_archive::archive::format::{detect_for_read, detect_for_write, Format};
use std::io::Write;

fn write_bytes(path: &std::path::Path, bytes: &[u8]) {
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(bytes).unwrap();
}

#[test]
fn detect_read_by_magic() {
    let dir = tempfile::tempdir().unwrap();
    let cases: &[(&str, &[u8], Format)] = &[
        ("a.zip", &[0x50, 0x4B, 0x03, 0x04], Format::Zip),
        ("a.gz", &[0x1F, 0x8B, 0x08, 0x00], Format::Gz),
        ("a.xz", &[0xFD, b'7', b'z', b'X', b'Z', 0x00], Format::Xz),
        ("a.zst", &[0x28, 0xB5, 0x2F, 0xFD], Format::Zst),
        ("a.bz2", &[0x42, 0x5A, 0x68, 0x39], Format::Bz2),
    ];
    for (name, magic, want) in cases {
        let p = dir.path().join(name);
        write_bytes(&p, magic);
        assert_eq!(detect_for_read(&p).unwrap(), *want, "{name}");
    }
}

#[test]
fn detect_write_by_extension() {
    assert_eq!(
        detect_for_write(std::path::Path::new("x.zip")).unwrap(),
        Format::Zip
    );
    assert_eq!(
        detect_for_write(std::path::Path::new("x.tar")).unwrap(),
        Format::Tar
    );
    assert_eq!(
        detect_for_write(std::path::Path::new("x.tar.gz")).unwrap(),
        Format::TarGz
    );
    assert_eq!(
        detect_for_write(std::path::Path::new("x.tgz")).unwrap(),
        Format::TarGz
    );
    assert_eq!(
        detect_for_write(std::path::Path::new("x.tar.zst")).unwrap(),
        Format::TarZst
    );
    assert_eq!(
        detect_for_write(std::path::Path::new("x.gz")).unwrap(),
        Format::Gz
    );
}
