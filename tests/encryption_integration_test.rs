use blitzarch::{compress, extract};
use rand::{thread_rng, Rng};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// Helper function to create a directory with some random files
fn create_test_data(
    dir: &Path,
    num_files: usize,
    file_size: usize,
) -> std::io::Result<Vec<PathBuf>> {
    fs::create_dir_all(dir)?;
    let mut paths = Vec::new();
    let mut rng = thread_rng();
    for i in 0..num_files {
        let file_path = dir.join(format!("file_{}.txt", i));
        let mut file = File::create(&file_path)?;
        let mut buffer = vec![0u8; file_size];
        rng.fill(&mut buffer[..]);
        file.write_all(&buffer)?;
        paths.push(file_path);
    }
    Ok(paths)
}

// Helper function to verify that two directories are identical
fn assert_dirs_equal(dir1: &Path, dir2: &Path) {
    let entries1: Vec<_> = fs::read_dir(dir1)
        .unwrap()
        .map(|r| r.unwrap().path())
        .filter(|p| p.is_file())
        .collect();
    let entries2: Vec<_> = fs::read_dir(dir2)
        .unwrap()
        .map(|r| r.unwrap().path())
        .filter(|p| p.is_file())
        .collect();

    assert_eq!(entries1.len(), entries2.len(), "Different number of files");

    for path1 in &entries1 {
        let file_name = path1.file_name().unwrap();
        let path2 = dir2.join(file_name);
        assert!(
            path2.exists(),
            "File {:?} does not exist in second dir",
            file_name
        );

        let content1 = fs::read(&path1).unwrap();
        let content2 = fs::read(&path2).unwrap();
        assert_eq!(
            content1,
            content2,
            "File contents differ for {:?}",
            file_name
        );
    }
}

#[test]
fn test_encrypted_archive_creation_and_extraction() {
    // 1. Setup
    let source_dir = tempdir().unwrap();
    create_test_data(source_dir.path(), 5, 1024).unwrap();

    let archive_dir = tempdir().unwrap();
    let archive_path = archive_dir.path().join("test_encrypted.blz");
    let password = "a_very_secret_password";

    // 2. Create encrypted archive
    let options = compress::CompressOptions {
        level: 3,
        threads: 1,

        text_bundle: blitzarch::cli::TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Zstd,
    };
    compress::run(
        &[source_dir.path().to_path_buf()],
        &archive_path,
        options,
        Some(password.to_string()),
    )
    .unwrap();

    assert!(archive_path.exists());

    // 3. Extract the archive
    let extract_dir = tempdir().unwrap();
    extract::extract_files(
        &archive_path,
        &[],
        Some(password),
        Some(extract_dir.path()),
        None, // strip_components
    )
    .unwrap();

    // 4. Verify correctness
    assert_dirs_equal(source_dir.path(), extract_dir.path());
}

#[test]
fn test_archive_with_empty_file() {
    // 1. Setup
    let source_dir = tempdir().unwrap();
    let empty_file_path = source_dir.path().join("empty.txt");
    File::create(&empty_file_path).unwrap();

    let archive_dir = tempdir().unwrap();
    let archive_path = archive_dir.path().join("test_empty_file.blz");

    // 2. Create archive (no encryption)
    let options = compress::CompressOptions {
        level: 3,
        threads: 1,

        text_bundle: blitzarch::cli::TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Zstd,
    };
    compress::run(
        &[source_dir.path().to_path_buf()],
        &archive_path,
        options,
        None,
    )
    .unwrap();

    // 3. Extract the archive
    let extract_dir = tempdir().unwrap();
    extract::extract_files(&archive_path, &[], None, Some(extract_dir.path()), None).unwrap();

    // 4. Verify correctness
    assert_dirs_equal(source_dir.path(), extract_dir.path());
    let extracted_empty_file = extract_dir.path().join("empty.txt");
    assert!(extracted_empty_file.exists());
    assert_eq!(fs::metadata(extracted_empty_file).unwrap().len(), 0);
}

#[test]
fn test_archive_with_empty_directory() {
    // 1. Setup
    let source_dir = tempdir().unwrap();
    let empty_dir_path = source_dir.path().join("empty_dir");
    fs::create_dir(&empty_dir_path).unwrap();

    let archive_dir = tempdir().unwrap();
    let archive_path = archive_dir.path().join("test_empty_dir.blz");

    // 2. Create archive
    let options = compress::CompressOptions {
        level: 3,
        threads: 1,

        text_bundle: blitzarch::cli::TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Zstd,
    };
    compress::run(
        &[source_dir.path().to_path_buf()],
        &archive_path,
        options,
        None,
    )
    .unwrap();

    // 3. Extract the archive
    let extract_dir = tempdir().unwrap();
    extract::extract_files(&archive_path, &[], None, Some(extract_dir.path()), None).unwrap();

    // 4. Verify correctness
    let extracted_empty_dir = extract_dir.path().join("empty_dir");
    assert!(extracted_empty_dir.exists());
    assert!(extracted_empty_dir.is_dir());
}

#[test]
fn test_extraction_fails_with_wrong_password() {
    // 1. Setup
    let source_dir = tempdir().unwrap();
    create_test_data(source_dir.path(), 2, 128).unwrap();
    let archive_dir = tempdir().unwrap();
    let archive_path = archive_dir.path().join("test_wrong_pass.blz");
    let correct_password = "correct_password";
    let wrong_password = "wrong_password";

    // 2. Create archive
    let options = compress::CompressOptions {
        level: 3,
        threads: 1,

        text_bundle: blitzarch::cli::TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Zstd,
    };
    compress::run(
        &[source_dir.path().to_path_buf()],
        &archive_path,
        options,
        Some(correct_password.to_string()),
    )
    .unwrap();

    // 3. Attempt extraction with wrong password
    let extract_dir = tempdir().unwrap();
    let result = extract::extract_files(
        &archive_path,
        &[],
        Some(wrong_password),
        Some(extract_dir.path()),
        None, // strip_components
    );

    // 4. Verify failure
    assert!(result.is_err());
    let error_msg = result.err().unwrap().to_string();
    assert!(
        error_msg.contains("Decryption failed"),
        "Unexpected error message: {}",
        error_msg
    );
}

#[test]
fn test_extraction_fails_without_password_for_encrypted_archive() {
    // 1. Setup
    let source_dir = tempdir().unwrap();
    create_test_data(source_dir.path(), 2, 128).unwrap();
    let archive_dir = tempdir().unwrap();
    let archive_path = archive_dir.path().join("test_no_pass.blz");
    let password = "a_password";

    // 2. Create archive
    let options = compress::CompressOptions {
        level: 3,
        threads: 1,

        text_bundle: blitzarch::cli::TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
        algo: compress::CompressionAlgo::Zstd,
    };
    compress::run(
        &[source_dir.path().to_path_buf()],
        &archive_path,
        options,
        Some(password.to_string()),
    )
    .unwrap();

    // 3. Attempt extraction with no password
    let extract_dir = tempdir().unwrap();
    let result = extract::extract_files(&archive_path, &[], None, Some(extract_dir.path()), None);

    // 4. Verify failure
    assert!(result.is_err());
    let error_msg = result.err().unwrap().to_string();
    assert!(
        error_msg.contains("Archive is encrypted, but no password was provided."),
        "Unexpected error message: {}",
        error_msg
    );
}
