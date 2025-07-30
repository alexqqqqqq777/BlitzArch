//! Integration test: ensure CLI auto-sanitizes invalid Windows filename characters
//! so that `blitzarch create` succeeds even if the provided --output path contains
//! < > : " / \ | ? * or reserved DOS names. The test is built & executed only on
//! Windows CI runners.

#![cfg(windows)]

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::tempdir;
use std::fs;

#[test]
fn test_cli_create_with_invalid_windows_path() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Prepare source directory with a tiny file
    let source_dir = tempdir()?;
    let dummy_path = source_dir.path().join("data.txt");
    fs::write(&dummy_path, b"dummy data")?;

    // 2. Prepare intentionally invalid archive filename
    let out_dir = tempdir()?;
    // Contains characters that are invalid on Windows file systems
    let bad_name = out_dir.path().join("bad<name>|?:archive.blz");

    // 3. Run `blitzarch create` with the invalid output path
    let mut cmd = Command::cargo_bin("blitzarch")?;
    cmd.arg("create")
        .arg("--output").arg(&bad_name)
        // bundle-size required by CLI (MiB)
        .arg("--bundle-size").arg("64")
        .arg(source_dir.path());
    cmd.assert().success();

    // 4. Verify that some .blz file exists in out_dir (sanitized by CLI)
    let mut blz_found = false;
    for entry in fs::read_dir(out_dir.path())? {
        let path = entry?.path();
        if path.extension().map_or(false, |ext| ext == "blz") {
            blz_found = true;
            // file should be non-zero in size
            assert!(fs::metadata(&path)?.len() > 0);
        }
    }
    assert!(blz_found, "No .blz archive created after sanitization");

    Ok(())
}
