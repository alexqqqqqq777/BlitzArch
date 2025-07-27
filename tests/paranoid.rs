use blitzarch::katana_stream::{create_katana_archive, perform_paranoid_check};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

fn create_sample_file(dir: &std::path::Path, name: &str, contents: &[u8]) -> PathBuf {
    let path = dir.join(name);
    let mut f = File::create(&path).expect("create sample file");
    f.write_all(contents).unwrap();
    path
}

#[test]
fn paranoid_ok() {
    let dir = tempdir().unwrap();
    let input = create_sample_file(dir.path(), "foo.txt", b"hello world");
    let out_path = dir.path().join("archive.katana");

    // create archive
    create_katana_archive(
        &[input],
        &out_path,
        /*threads*/ 1,
        /*codec_threads*/ 1,
        /*mem_budget*/ None,
        /*password*/ None,
    )
    .expect("create archive");

    // paranoid check should pass
    perform_paranoid_check(&out_path).expect("paranoid ok");
}

#[test]
fn paranoid_corrupt() {
    let dir = tempdir().unwrap();
    let input = create_sample_file(dir.path(), "bar.txt", b"hello corrupted");
    let out_path = dir.path().join("archive_corrupt.katana");

    create_katana_archive(&[input], &out_path, 1, 1, None, None, None, None::<fn(crate::progress::ProgressState)>).expect("create archive");

    // corrupt the archive by appending junk
    {
        let mut f = OpenOptions::new().write(true).append(true).open(&out_path).unwrap();
        f.write_all(b"junk").unwrap();
    }

    // paranoid check should fail
    assert!(perform_paranoid_check(&out_path).is_err());
}
