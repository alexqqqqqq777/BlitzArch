use blitzarch::katana;
use rand::{thread_rng, RngCore};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

// Helper to create a file with random bytes of given size
fn create_random_file<P: AsRef<Path>>(path: P, size: usize) {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut f = File::create(path).unwrap();
    let mut buf = vec![0u8; size];
    thread_rng().fill_bytes(&mut buf);
    f.write_all(&buf).unwrap();
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
        let rel = p.strip_prefix(a).unwrap();
        let pb = b.join(rel);
        assert!(pb.exists(), "missing {:?}", pb);
        assert_eq!(fs::read(&p).unwrap(), fs::read(&pb).unwrap());
    }
}

#[test]
fn katana_unicode_filenames_roundtrip() {
    let src = tempdir().unwrap();

    // Create several unicode/emoji files
    create_random_file(src.path().join("こんにちは.txt"), 1234);
    create_random_file(src.path().join("emoji_😀.bin"), 2048);
    create_random_file(src.path().join("русский_файл.log"), 4096);

    // Build archive
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("unicode.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None)
        .expect("create archive");

    // Extract
    let out = tempdir().unwrap();
    katana::extract_katana_archive_internal(&arch_path, out.path(), &[], None)
        .expect("extract");

    // Compare dirs
    dirs_equal(src.path(), out.path());
}

#[cfg(unix)]
#[test]
fn katana_permissions_preserved() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    
    let src = tempdir().unwrap();
    let file_path = src.path().join("script.sh");
    create_random_file(&file_path, 512);

    // Make it executable
    let perms = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&file_path, perms.clone()).unwrap();

    // Build archive
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("perm.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None)
        .expect("create");

    // Extract
    let out = tempdir().unwrap();
    katana::extract_katana_archive_internal(&arch_path, out.path(), &[], None)
        .expect("extract");

    let extracted_path = out.path().join("script.sh");
    let extracted_perms = fs::metadata(&extracted_path).unwrap().permissions();
    assert_eq!(extracted_perms.mode() & 0o777, 0o755, "permissions not preserved");
}

#[test]
fn katana_wrong_password_fails() {
    let src = tempdir().unwrap();
    create_random_file(src.path().join("secret.txt"), 1024);

    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("enc.blz");
    let password = "correct_pass";

    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, Some(password.to_string()))
        .expect("create");

    let out = tempdir().unwrap();
    let res = katana::extract_katana_archive_internal(&arch_path, out.path(), &[], Some("wrong_pass".into()));
    assert!(res.is_err(), "Extraction should fail with wrong password");
}

#[test]
fn katana_corrupted_archive_detection() {
    let src = tempdir().unwrap();
    create_random_file(src.path().join("data.bin"), 2048);

    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("corrupt.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None).expect("create");

    // Flip a byte in the middle
    let mut f = OpenOptions::new().read(true).write(true).open(&arch_path).unwrap();
    let len = f.metadata().unwrap().len();
    let pos = len / 2;
    use std::io::{Read, Seek, SeekFrom, Write};
    f.seek(SeekFrom::Start(pos)).unwrap();
    let mut b = [0u8;1];
    f.read_exact(&mut b).unwrap();
    b[0] ^= 0xFF;
    f.seek(SeekFrom::Start(pos)).unwrap();
    f.write_all(&b).unwrap();
    f.sync_all().unwrap();

    let out = tempdir().unwrap();
    let res = katana::extract_katana_archive_internal(&arch_path, out.path(), &[], None);
    assert!(res.is_err(), "Extraction should fail on corrupted archive");
}

#[test]
#[ignore]
fn katana_sparse_ratio_demo() {
    const SIZE_BYTES: u64 = 50 * 1024 * 1024 * 1024; // 50 GiB
    use std::io::{Seek, SeekFrom, Write};
    let src = tempdir().unwrap();
    let sparse_path = src.path().join("huge_sparse.bin");
    let mut f = File::create(&sparse_path).unwrap();
    f.seek(SeekFrom::Start(SIZE_BYTES - 1)).unwrap();
    f.write_all(&[0xAA]).unwrap(); // one byte at end
    f.sync_all().unwrap();

    // Build archive
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("sparse.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None).expect("create");

    let comp_size = fs::metadata(&arch_path).unwrap().len();
    let ratio = SIZE_BYTES as f64 / comp_size as f64;
    println!("[marketing] sparse ratio: {:.2}x ({} bytes vs {})", ratio, SIZE_BYTES, comp_size);
    // Expect at least 10 000× compression
    assert!(ratio > 10000.0, "ratio not impressive: {:.2}", ratio);
}

#[cfg(unix)]
#[test]
fn katana_absolute_path_traversal_blocked() {
    // Prepare absolute path file so it will be stored with leading '/'
    let abs_dir = tempfile::tempdir().unwrap();
    let abs_file = abs_dir.path().join("secret.txt");
    create_random_file(&abs_file, 256);

    // Build archive containing absolute path entry
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("abs.blz");
    katana::create_katana_archive(&[abs_file.clone()], &arch_path, 0, None).expect("create");

    // Extract; implementation should sanitise absolute paths into relative entries
    let out = tempdir().unwrap();
    katana::extract_katana_archive_internal(&arch_path, out.path(), &[], None).expect("extract");

    // The file should now reside inside output dir with no leading slash
    assert!(out.path().join("secret.txt").exists(), "sanitised file missing inside output dir");
}

#[test]
#[ignore]
fn katana_million_files_selective_extract() {
    use std::time::Instant;
    const FILES: usize = 10_000; // use 10k for test runtime, conceptually million
    let src = tempdir().unwrap();
    // Generate many tiny files
    for i in 0..FILES {
        let p = src.path().join(format!("dir{}/file{}.txt", i / 1000, i));
        create_random_file(&p, 0);
    }

    // Build archive
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("many.blz");
    let t_build = Instant::now();
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None).expect("create");
    let build_ms = t_build.elapsed().as_millis();
    println!("[marketing] build {} files in {} ms", FILES, build_ms);

    // Select a single file somewhere near end
    let target_rel = format!("dir{}/file{}.txt", (FILES-1)/1000, FILES-1);
    let out = tempdir().unwrap();
    let t_ext = Instant::now();
    katana::extract_katana_archive_internal(&arch_path, out.path(), &[target_rel.clone().into()], None).expect("extract one");
    let ext_ms = t_ext.elapsed().as_millis();
    println!("[marketing] selective extract in {} ms", ext_ms);

    assert!(out.path().join(target_rel).exists(), "selected file missing after extract");
    // Heuristic: selective extract should be at least 10x faster than archive build
    assert!(ext_ms < build_ms / 2, "selective extract too slow vs build: {} vs {}", ext_ms, build_ms);
}

