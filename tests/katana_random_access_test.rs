use blitzarch::katana;
use rand::{thread_rng, Rng};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn write_random_file(p: &Path, sz: usize) {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut f = File::create(p).unwrap();
    let mut buf = vec![0u8; sz];
    thread_rng().fill(&mut buf[..]);
    f.write_all(&buf).unwrap();
}

#[test]
fn katana_random_access_extract() {
    let src = tempdir().unwrap();
    // Create files: a.txt, dir/b.bin, dir/c.log
    write_random_file(&src.path().join("a.txt"), 1500);
    write_random_file(&src.path().join("dir/b.bin"), 3000);
    write_random_file(&src.path().join("dir/c.log"), 1024);

    // Build archive
    let arch_dir = tempdir().unwrap();
    let arch_path = arch_dir.path().join("test.blz");
    katana::create_katana_archive(&[src.path().to_path_buf()], &arch_path, 0, None).unwrap();

    // We want to extract only dir/c.log
    let want_rel: PathBuf = PathBuf::from("dir/c.log");
    let out = tempdir().unwrap();
    katana::extract_katana_archive_internal(&arch_path, out.path(), &[want_rel.clone()], None, None).unwrap();

    // Check only wanted file exists and matches
    let extracted_path = out.path().join(&want_rel);
    assert!(extracted_path.exists());
    assert_eq!(
        fs::read(src.path().join(&want_rel)).unwrap(),
        fs::read(&extracted_path).unwrap()
    );

    // Ensure other files were NOT extracted
    assert!(!out.path().join("a.txt").exists());
    assert!(!out.path().join("dir/b.bin").exists());
}
