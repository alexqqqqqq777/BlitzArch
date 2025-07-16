use blitzarch::katana;
use rand::{thread_rng, RngCore};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use tempfile::tempdir;

// Helper: create N random files of given size under dir
fn create_files(dir: &Path, n: usize, size: usize) {
    fs::create_dir_all(dir).unwrap();
    let mut rng = thread_rng();
    for i in 0..n {
        let path = dir.join(format!("f{}.bin", i));
        let mut f = File::create(&path).unwrap();
        let mut buf = vec![0u8; size];
        rng.fill_bytes(&mut buf);
        f.write_all(&buf).unwrap();
    }
}

fn dirs_equal(a: &Path, b: &Path) {
    let list = |d: &Path| {
        fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_file())
            .collect::<Vec<_>>()
    };
    let la = list(a);
    let lb = list(b);
    assert_eq!(la.len(), lb.len());
    for p in la {
        let name = p.file_name().unwrap();
        let pb = b.join(name);
        assert!(pb.exists());
        assert_eq!(fs::read(p).unwrap(), fs::read(pb).unwrap());
    }
}

#[test]
fn katana_unicode_password_roundtrip() {
    let src = tempdir().unwrap();
    create_files(src.path(), 3, 1024);

    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("unicode.enc.blz");
    let password = "🔑Пароль🚀✨"; // unicode + emoji

    katana::create_katana_archive(
        &[src.path().to_path_buf()],
        &arch_path,
        0,
        Some(password.to_string()),
    )
    .expect("archive create");

    let out = tempdir().unwrap();
    katana::extract_katana_archive_internal(
        &arch_path,
        out.path(),
        &[],
        Some(password.to_string()),
    )
    .expect("extract");

    dirs_equal(src.path(), out.path());
}

#[test]
fn katana_corrupted_shard_detection() {
    let src = tempdir().unwrap();
    create_files(src.path(), 2, 2048);
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("corrupt.blz");

    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None)
        .expect("create");

    // Flip a byte roughly in the middle of the file
    let mut file = OpenOptions::new().read(true).write(true).open(&arch_path).unwrap();
    let len = file.metadata().unwrap().len();
    let pos = len / 2;
    file.seek(SeekFrom::Start(pos)).unwrap();
    let mut byte = [0u8; 1];
    file.read_exact(&mut byte).unwrap();
    byte[0] ^= 0xFF; // invert bits
    file.seek(SeekFrom::Start(pos)).unwrap();
    file.write_all(&byte).unwrap();
    file.sync_all().unwrap();

    let out = tempdir().unwrap();
    let res = katana::extract_katana_archive_internal(&arch_path, out.path(), &[], None);
    assert!(res.is_err(), "Extraction should fail on corrupted archive");
}
