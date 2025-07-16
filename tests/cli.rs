use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::tempdir;
use std::fs;
use std::io::Write;

#[test]
fn test_cli_create_list_extract_cycle() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup: Create a temporary directory and some test files
    let source_dir = tempdir()?;
    let file1_path = source_dir.path().join("file1.txt");
    let file2_path = source_dir.path().join("file2.log");
    let nested_dir = source_dir.path().join("nested");
    fs::create_dir(&nested_dir)?;
    let nested_file_path = nested_dir.join("nested_file.dat");

    let mut file1 = fs::File::create(&file1_path)?;
    writeln!(file1, "Hello, this is the first file.")?;

    let mut file2 = fs::File::create(&file2_path)?;
    writeln!(file2, "Some log data here.")?;

    let mut nested_file = fs::File::create(&nested_file_path)?;
    nested_file.write_all(&[0, 1, 2, 3, 4, 5])?;

    let archive_dir = tempdir()?;
    let archive_path = archive_dir.path().join("test_archive.mfa");

    // 2. Create archive
    let mut cmd = Command::cargo_bin("blitzarch")?;
    cmd.arg("create")
        .arg("--output")
        .arg(&archive_path)
        .arg("--bundle-size")
        .arg("65536")
        .arg(source_dir.path());
    cmd.assert().success();

    assert!(archive_path.exists());

    // 3. List contents of the archive
    let mut cmd = Command::cargo_bin("blitzarch")?;
    cmd.arg("list").arg(&archive_path);
    cmd.assert()
        .success()
        .stdout(
            predicate::str::contains("file1.txt")
            .and(predicate::str::contains("file2.log"))
            .and(predicate::str::contains("nested_file.dat"))
        );

    // 4. Extract archive to a new directory
    let extract_dir = tempdir()?;
    let mut cmd = Command::cargo_bin("blitzarch")?;
    cmd.arg("extract")
        .arg(&archive_path)
        .arg("-o")
        .arg(extract_dir.path());
    cmd.assert().success();

    // 5. Verify extracted files
    let extracted_file1 = fs::read(extract_dir.path().join("file1.txt"))?;
    let original_file1 = fs::read(&file1_path)?;
    assert_eq!(extracted_file1, original_file1);

    let extracted_file2 = fs::read(extract_dir.path().join("file2.log"))?;
    let original_file2 = fs::read(&file2_path)?;
    assert_eq!(extracted_file2, original_file2);

    let extracted_nested_file = fs::read(extract_dir.path().join("nested/nested_file.dat"))?;
    let original_nested_file = fs::read(&nested_file_path)?;
    assert_eq!(extracted_nested_file, original_nested_file);

    Ok(())
}
