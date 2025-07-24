use blitzarch::katana;
use rand::{thread_rng, Rng};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

fn create_test_files(dir: &Path, n: usize, sz: usize) {
    fs::create_dir_all(dir).unwrap();
    let mut rng = thread_rng();
    for i in 0..n {
        let p = dir.join(format!("f{}.dat", i));
        let mut f = File::create(&p).unwrap();
        let mut buf = vec![0u8; sz];
        rng.fill(&mut buf[..]);
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
fn katana_roundtrip_basic() {
    let src = tempdir().unwrap();
    create_test_files(src.path(), 6, 4096);

    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("test.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None).unwrap();

    assert!(katana::is_katana_archive(&arch_path).unwrap());

    let out = tempdir().unwrap();
    blitzarch::extract::extract_files(&arch_path, &[], None, Some(out.path()), None).unwrap();
    dirs_equal(src.path(), out.path());
}

#[test]
fn katana_detection_false_for_regular_archive() {
    // create small regular mfa archive then verify detection
    let src = tempdir().unwrap();
    create_test_files(src.path(), 2, 1024);
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("reg.blz");
    let opts = blitzarch::compress::CompressOptions {
        level: 1,
        threads: 1,
        text_bundle: blitzarch::cli::TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: blitzarch::compress::CompressionAlgo::Zstd,
    };
    blitzarch::compress::run(&[src.path().to_path_buf()], &arch_path, opts, None).unwrap();
    assert_eq!(katana::is_katana_archive(&arch_path).unwrap(), false);
}
