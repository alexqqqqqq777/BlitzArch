use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant};

use rand::seq::SliceRandom;
use rand::thread_rng;
use walkdir::WalkDir;

const DATASET_PATH: &str = "/Users/oleksandr/Desktop/Development/BTSL/DATASET";
const NUM_RANDOM_FILES: usize = 1;

#[derive(Debug)]
struct BenchResult {
    archiver: String,
    operation: String,
    avg_wall_time_ms: f64,
}

impl std::fmt::Display for BenchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "| {:<12} | {:<18} | {:>21.3} |",
            self.archiver,
            self.operation,
            self.avg_wall_time_ms
        )
    }
}

fn get_mfa_executable_path() -> Result<PathBuf, Box<dyn Error>> {
    // The binary should already be built by `cargo run`
    let path = PathBuf::from("target/release/aatrnnbdye");
    if !path.exists() {
        return Err("MFA executable not found. Please run `cargo build --release` first.".into());
    }
    Ok(path)
}

/// Runs a command and returns its execution duration.
fn run_command(mut command: Command) -> Result<Instant, Box<dyn Error>> {
    let start_time = Instant::now();
    let status = command.stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    if !status.success() {
        return Err(format!("Command failed: {:?}", command).into());
    }
    Ok(start_time)
}

fn setup_archives() -> Result<(PathBuf, PathBuf, PathBuf, PathBuf, PathBuf), Box<dyn Error>> {
    println!("--- Setting up Test Archives (Level 7) ---");
    let archive_dir = PathBuf::from("benches/test_archives");
    fs::create_dir_all(&archive_dir)?;

    let mfa_archive = archive_dir.join("dataset_l7.mfa");
    let tar_archive = archive_dir.join("dataset_l7.tar.zst");
    let zip_archive = archive_dir.join("dataset_l7_zip.zip");
    let lzma2_archive = archive_dir.join("dataset_l7.7z");

    let mfa_executable = get_mfa_executable_path()?;

    // MFA Archive
    if !mfa_archive.exists() {
        println!("Creating MFA archive...");
        let mut cmd = Command::new(&mfa_executable);
        cmd.args([
            "create",
            "--level",
            "7",
            "--text-bundle",
            "auto",
            "--output",
            mfa_archive.to_str().unwrap(),
            DATASET_PATH,
        ]);
        run_command(cmd)?;
    } else {
        println!("Found existing MFA archive.");
    }

    // tar+zstd Archive
    if !tar_archive.exists() {
        println!("Creating tar+zstd archive...");
        let tar_file = tar_archive.with_extension("");
        let mut cmd_tar = Command::new("tar");
        cmd_tar.args([
            "--create",
            "--file",
            tar_file.to_str().unwrap(),
            "-C",
            DATASET_PATH,
            ".",
        ]);
        run_command(cmd_tar)?;

        let mut cmd_zstd = Command::new("zstd");
        cmd_zstd.args([
            "-7",
            "-T0",
            tar_file.to_str().unwrap(),
            "-o",
            tar_archive.to_str().unwrap(),
        ]);
        run_command(cmd_zstd)?;
        fs::remove_file(tar_file)?;
    } else {
        println!("Found existing tar+zstd archive.");
    }

    // zip+zstd Archive
    if !zip_archive.exists() {
        println!("Creating zip+zstd archive...");
        let mut cmd = Command::new("7z");
        cmd.args([
            "a",
            "-tzip",
            "-mm=zstd",
            "-mx=7",
            zip_archive.to_str().unwrap(),
            &format!("{}/.", DATASET_PATH),
        ]);
        run_command(cmd)?;
    } else {
        println!("Found existing zip+zstd archive.");
    }

    // 7z (LZMA2) Archive
    if !lzma2_archive.exists() {
        println!("Creating 7z+lzma2 archive...");
        let mut cmd = Command::new("7z");
        cmd.args([
            "a",
            "-t7z",
            "-m0=lzma2",
            "-mx=7",
            lzma2_archive.to_str().unwrap(),
            &format!("{}/.", DATASET_PATH),
        ]);
        run_command(cmd)?;
    } else {
        println!("Found existing 7z+lzma2 archive.");
    }

    // MFA Archive (Small Bundles)
    let mfa_archive_small = archive_dir.join("dataset_l7_small_bundle.mfa");
    if !mfa_archive_small.exists() {
        println!("Creating MFA archive (small bundles)...");
        let mut cmd = Command::new(&mfa_executable);
        cmd.args([
            "create",
            "--level",
            "7",
            "--text-bundle",
            "small",
            "--output",
            mfa_archive_small.to_str().unwrap(),
            DATASET_PATH,
        ]);
        run_command(cmd)?;
    } else {
        println!("Found existing MFA archive (small bundles).");
    }

    println!("--- Setup Complete ---");
    Ok((mfa_archive, mfa_archive_small, tar_archive, zip_archive, lzma2_archive))
}

fn get_random_file_list() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let all_files: Vec<PathBuf> = WalkDir::new(DATASET_PATH)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(DATASET_PATH).unwrap().to_path_buf())
        .collect();

    let mut rng = thread_rng();
    let random_files: Vec<PathBuf> = all_files
        .choose_multiple(&mut rng, NUM_RANDOM_FILES)
        .cloned()
        .collect();

    Ok(random_files)
}

fn main() -> Result<(), Box<dyn Error>> {
    let (mfa_archive_auto, mfa_archive_small, _, _, _) = setup_archives()?;
    let random_files = get_random_file_list()?;

    if random_files.is_empty() {
        println!("No random files selected. Exiting.");
        return Ok(());
    }

    let file_to_extract = &random_files[0];
    println!("\n--- Running Single File Extraction Test for MFA ---");
    println!("Target File: {:?}", file_to_extract);

    let mfa_executable = get_mfa_executable_path()?;
    let extract_dir = PathBuf::from("benches/test_archives/mfa_extracted");

    // --- Test 1: 'auto' bundle archive ---
    fs::create_dir_all(&extract_dir)?;
    let mut cmd_auto = Command::new(&mfa_executable);
    cmd_auto.args([
        "extract",
        "--output",
        extract_dir.to_str().unwrap(),
        mfa_archive_auto.to_str().unwrap(),
        file_to_extract.to_str().unwrap(),
    ]);

    let start_auto = Instant::now();
    let status_auto = cmd_auto.stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    if !status_auto.success() {
        return Err(format!("Command failed for 'auto' bundle: {:?}", cmd_auto).into());
    }
    let duration_auto = start_auto.elapsed();
    fs::remove_dir_all(&extract_dir)?;

    // --- Test 2: 'small' bundle archive ---
    fs::create_dir_all(&extract_dir)?;
    let mut cmd_small = Command::new(&mfa_executable);
    cmd_small.args([
        "extract",
        "--output",
        extract_dir.to_str().unwrap(),
        mfa_archive_small.to_str().unwrap(),
        file_to_extract.to_str().unwrap(),
    ]);

    let start_small = Instant::now();
    let status_small = cmd_small.stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    if !status_small.success() {
        return Err(format!("Command failed for 'small' bundle: {:?}", cmd_small).into());
    }
    let duration_small = start_small.elapsed();
    fs::remove_dir_all(&extract_dir)?;

    // --- Results ---
    println!("\n--- Test Results ---");
    println!("Time with 'auto' bundles:  {:?}", duration_auto);
    println!("Time with 'small' bundles: {:?}", duration_small);

    Ok(())
}
