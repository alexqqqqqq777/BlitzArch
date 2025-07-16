use blitzarch::{compress, cli::TextBundleMode};
use rand::{thread_rng, Rng};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// ---------- helpers ----------
fn create_test_data(dir: &Path, num_files: usize, file_size: usize) -> std::io::Result<Vec<PathBuf>> {
    fs::create_dir_all(dir)?;
    let mut paths = Vec::new();
    let mut rng = thread_rng();
    for i in 0..num_files {
        let file_path = dir.join(format!("file_{}.bin", i));
        let mut file = File::create(&file_path)?;
        let mut buf = vec![0u8; file_size];
        rng.fill(&mut buf[..]);
        file.write_all(&buf)?;
        paths.push(file_path);
    }
    Ok(paths)
}

fn assert_dirs_equal(dir1: &Path, dir2: &Path) {
    let list = |d: &Path| -> Vec<PathBuf> {
        fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_file())
            .collect::<Vec<_>>()
    };
    let entries1 = list(dir1);
    let entries2 = list(dir2);
    assert_eq!(entries1.len(), entries2.len(), "Different number of files");

    for p1 in &entries1 {
        let name = p1.file_name().unwrap();
        let p2 = dir2.join(name);
        assert!(p2.exists(), "File {:?} missing in extracted dir", name);
        let c1 = fs::read(p1).unwrap();
        let c2 = fs::read(p2).unwrap();
        assert_eq!(c1, c2, "Content mismatch for {:?}", name);
    }
}

fn roundtrip(options: compress::CompressOptions, password: Option<&str>) {
    let src_dir = tempdir().unwrap();
    create_test_data(src_dir.path(), 5, 2048).unwrap();

    let archive_dir = tempdir().unwrap();
    let archive_path = archive_dir.path().join("test.mfa");

    compress::run(
        &[src_dir.path().to_path_buf()],
        &archive_path,
        options.clone(),
        password.map(|s| s.to_string()),
    )
    .expect("compression failed");

    let out_dir = tempdir().unwrap();
    blitzarch::extract::extract_files(
        &archive_path,
        &[],
        password,
        Some(out_dir.path()),
    )
    .expect("extraction failed");

    assert_dirs_equal(src_dir.path(), out_dir.path());
}

#[test]
fn roundtrip_zstd() {
    let opts = compress::CompressOptions {
        level: 3,
        threads: 2,
        text_bundle: TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Zstd,
    };
    roundtrip(opts, None);
}

#[test]
fn roundtrip_store() {
    let opts = compress::CompressOptions {
        level: 0,
        threads: 1,
        text_bundle: TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Store,
    };
    roundtrip(opts, None);
}

#[test]
fn roundtrip_lzma2() {
    let opts = compress::CompressOptions {
        level: 7,
        threads: 4,
        text_bundle: TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Lzma2 { preset: 7 },
    };
    roundtrip(opts, None);
}

#[test]
fn roundtrip_zstd_encrypted() {
    let opts = compress::CompressOptions {
        level: 3,
        threads: 2,
        text_bundle: TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Zstd,
    };
    let pwd = "secret_pass";
    roundtrip(opts, Some(pwd));
}

#[test]
fn random_access_extract() {
    // create 10 files
    let src_dir = tempdir().unwrap();
    create_test_data(src_dir.path(), 10, 1024).unwrap();

    let arch = tempdir().unwrap();
    let arch_path = arch.path().join("ra.mfa");
    let opts = compress::CompressOptions {
        level: 3,
        threads: 2,
        text_bundle: TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Zstd,
    };
    compress::run(&[src_dir.path().to_path_buf()], &arch_path, opts, None).unwrap();

    // pick 3 random file names
    let mut all_files: Vec<_> = fs::read_dir(src_dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    all_files.sort();
    let selected: Vec<_> = all_files.iter().take(3).map(PathBuf::from).collect();

    let out_dir = tempdir().unwrap();
    blitzarch::extract::extract_files(&arch_path, &selected, None, Some(out_dir.path())).unwrap();

    // only selected files should exist
    let extracted: Vec<_> = fs::read_dir(out_dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(extracted.len(), selected.len());
    for name in &selected {
        let extracted_path = out_dir.path().join(name);
        assert!(extracted_path.exists());
        assert_eq!(fs::read(src_dir.path().join(name)).unwrap(), fs::read(extracted_path).unwrap());
    }
}

#[test]
fn zstd_wrong_password_fails() {
    let opts = compress::CompressOptions {
        level: 3,
        threads: 2,
        text_bundle: TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Zstd,
    };

    let src_dir = tempdir().unwrap();
    create_test_data(src_dir.path(), 3, 1024).unwrap();
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("enc.mfa");
    compress::run(
        &[src_dir.path().to_path_buf()],
        &arch_path,
        opts,
        Some("correct".into()),
    )
    .unwrap();

    let out = tempdir().unwrap();
    let res = blitzarch::extract::extract_files(&arch_path, &[], Some("wrong"), Some(out.path()));
    assert!(res.is_err(), "Extraction with wrong password should fail");
}
