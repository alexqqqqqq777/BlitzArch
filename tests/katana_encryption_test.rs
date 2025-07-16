use blitzarch::katana;
use rand::{thread_rng, Rng};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

fn create_random_files(dir: &Path, n: usize, sz: usize) {
    fs::create_dir_all(dir).unwrap();
    let mut rng = thread_rng();
    for i in 0..n {
        let path = dir.join(format!("file_{}.bin", i));
        let mut f = File::create(&path).unwrap();
        let mut buf = vec![0u8; sz];
        rng.fill(&mut buf[..]);
        f.write_all(&buf).unwrap();
    }
}

fn compare_dirs(a: &Path, b: &Path) {
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
fn katana_roundtrip_encrypted() {
    let src_dir = tempdir().unwrap();
    create_random_files(src_dir.path(), 4, 2048);

    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("enc.blz");
    let password = "correct horse battery staple";

    // Create encrypted archive
    katana::create_katana_archive(
        &[src_dir.path().to_path_buf()],
        &arch_path,
        0,
        Some(password.to_string()),
    )
    .expect("archive creation failed");

    // Extract with correct password
    let out_dir = tempdir().unwrap();
    katana::extract_katana_archive_internal(
        &arch_path,
        out_dir.path(),
        &[],
        Some(password.to_string()),
    )
    .expect("extraction failed");

    compare_dirs(src_dir.path(), out_dir.path());
}

#[test]
fn katana_wrong_password() {
    let src_dir = tempdir().unwrap();
    create_random_files(src_dir.path(), 2, 1024);

    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("enc.blz");
    let password = "password123";

    katana::create_katana_archive(
        &[src_dir.path().to_path_buf()],
        &arch_path,
        0,
        Some(password.to_string()),
    )
    .unwrap();

    // Attempt extraction with wrong password should fail
    let out_dir = tempdir().unwrap();
    let result = katana::extract_katana_archive_internal(
        &arch_path,
        out_dir.path(),
        &[],
        Some("wrong_pass".to_string()),
    );

    assert!(result.is_err(), "Extraction should fail with wrong password");
}
