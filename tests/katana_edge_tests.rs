//! Extensive edge-case tests for Katana archives.
//! Heavy tests are marked with `#[ignore]` so CI can skip them by default.

use blitzarch::katana;
use rand::{distributions::{Alphanumeric, Distribution}, rngs::ThreadRng, thread_rng, Rng, RngCore};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use tempfile::tempdir;

// ---------- helpers ----------

fn create_files(dir: &Path, n: usize, size: usize, rng: &mut ThreadRng) {
    fs::create_dir_all(dir).unwrap();
    let mut buf = vec![0u8; size];
    for i in 0..n {
        let path = dir.join(format!("file_{i}.bin"));
        rng.fill_bytes(&mut buf);
        File::create(&path).unwrap().write_all(&buf).unwrap();
    }
}

fn dirs_equal(a: &Path, b: &Path) {
    let walk = |p: &Path| {
        fs::read_dir(p)
            .unwrap()
            .flat_map(|e| e)
            .filter(|e| e.path().is_file())
            .map(|e| e.path())
            .collect::<Vec<_>>()
    };
    let la = walk(a);
    let lb = walk(b);
    assert_eq!(la.len(), lb.len());
    for pa in &la {
        let rel = pa.file_name().unwrap();
        let pb = b.join(rel);
        assert!(pb.exists(), "missing {:?}", pb);
        assert_eq!(fs::read(pa).unwrap(), fs::read(pb).unwrap());
    }
}

// ---------- lightweight edge-case tests (run on CI) ----------

#[test]
fn katana_password_fuzz_ascii() {
    let mut rng = thread_rng();
    for _ in 0..30 {
        let len = rng.gen_range(1..40);
        let pwd: String = Alphanumeric
            .sample_iter(&mut rng)
            .take(len)
            .map(char::from)
            .collect();
        roundtrip_with_password(&pwd);
    }
}

#[test]
fn katana_password_fuzz_unicode() {
    let mut rng = thread_rng();
    let emoji = ["😺", "🚀", "✨", "🛡️", "🔒", "📦", "🎉", "🐉"];
    for _ in 0..10 {
        let mut pwd = String::new();
        let segments = rng.gen_range(1..6);
        for _ in 0..segments {
            if rng.gen_bool(0.5) {
                pwd.push_str(emoji[rng.gen_range(0..emoji.len())]);
            } else {
                let seg_len = rng.gen_range(1..8);
                let segment: String = Alphanumeric
                    .sample_iter(&mut rng)
                    .take(seg_len)
                    .map(char::from)
                    .collect();
                pwd.push_str(&segment);
            }
        }
        roundtrip_with_password(&pwd);
    }
}

fn roundtrip_with_password(password: &str) {
    let mut rng = thread_rng();
    let src = tempdir().unwrap();
    create_files(src.path(), 2, 1024, &mut rng);
    let arch = tempdir().unwrap();
    let arch_path = arch.path().join("fuzz.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, Some(password.to_string())).unwrap();
    let out = tempdir().unwrap();
    katana::extract_katana_archive_internal(&arch_path, out.path(), &[], Some(password.to_string()), None).unwrap();
    dirs_equal(src.path(), out.path());
}

#[test]
fn katana_truncated_footer() {
    let mut rng = thread_rng();
    let src = tempdir().unwrap();
    create_files(src.path(), 1, 512, &mut rng);
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("trunc.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None).unwrap();
    // Strip last 32 bytes (footer + CRC)
    let f = OpenOptions::new().read(true).write(true).open(&arch_path).unwrap();
    let len = f.metadata().unwrap().len();
    f.set_len(len - 32).unwrap();
    // Archive should now be invalid
    assert_eq!(katana::is_katana_archive(&arch_path).unwrap_or(false), false);
    let res = katana::extract_katana_archive_internal(&arch_path, src.path(), &[], None, None);
    assert!(res.is_err());
}

#[test]
fn katana_wrong_magic_bytes() {
    let mut rng = thread_rng();
    let src = tempdir().unwrap();
    create_files(src.path(), 1, 256, &mut rng);
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("magic.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None).unwrap();
    // Corrupt footer magic bytes (last 8 bytes)
    let mut f = OpenOptions::new().read(true).write(true).open(&arch_path).unwrap();
    let metadata = f.metadata().unwrap();
    let file_size = metadata.len();
    f.seek(SeekFrom::Start(file_size - 8)).unwrap();
    f.write_all(&[0u8; 8]).unwrap();
    f.sync_all().unwrap();
    // Detection should fail – no further extraction attempt needed
    assert_eq!(katana::is_katana_archive(&arch_path).unwrap_or(false), false);
}

// ---------- heavy stress tests (ignored by default) ----------

#[test]
#[ignore]
fn katana_sparse_huge_file() {
    // Create sparse 50 GiB file with a sentinel at the end.
    let src_dir = tempdir().unwrap();
    let file_path = src_dir.path().join("huge_sparse.bin");
    let mut f = File::create(&file_path).unwrap();
    const SIZE_BYTES: u64 = 50 * 1024 * 1024 * 1024; // 50 GiB
    f.seek(SeekFrom::Start(SIZE_BYTES - 1)).unwrap();
    f.write_all(&[0xAB]).unwrap(); // write one byte
    f.sync_all().unwrap();

    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("huge.blz");
    katana::create_katana_archive(&[src_dir.path().to_path_buf()], &arch_path, 0, None).unwrap();

    let out_dir = tempdir().unwrap();
    katana::extract_katana_archive_internal(&arch_path, out_dir.path(), &[], None, None).unwrap();

    // Verify size preserved and sentinel byte matches
    let out_file = out_dir.path().join("huge_sparse.bin");
    let meta = fs::metadata(&out_file).unwrap();
    assert_eq!(meta.len(), SIZE_BYTES);
    let mut last = [0u8; 1];
    let mut fo = File::open(&out_file).unwrap();
    fo.seek(SeekFrom::End(-1)).unwrap();
    fo.read_exact(&mut last).unwrap();
    assert_eq!(last[0], 0xAB);
}

#[test]
#[ignore]
fn katana_million_files_random_access() {
    let mut rng = thread_rng();
    let src = tempdir().unwrap();
    // Reduced to 10k for practical runtime; still heavy
    let count = 10_000u32;
    create_files(src.path(), count as usize, 128, &mut rng);
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("million.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None).unwrap();

    // Randomly sample 100 files for selective extract
    use rand::seq::SliceRandom;
    let files: Vec<_> = (0..count).map(|i| format!("file_{i}.bin")).collect();
    let sample: Vec<_> = files.choose_multiple(&mut rng, 100).cloned().collect();

    let out_dir = tempdir().unwrap();
    let sample_paths: Vec<_> = sample.iter().map(|s| Path::new(s).to_path_buf()).collect();
    katana::extract_katana_archive_internal(&arch_path, out_dir.path(), &sample_paths, None, None).unwrap();

    // Verify extracted subset
    for name in &sample {
        let original = src.path().join(name);
        let extracted = out_dir.path().join(name);
        assert_eq!(fs::read(original).unwrap(), fs::read(extracted).unwrap());
    }
}
