use std::fs::{self};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use jwalk::WalkDir;
use tempfile::tempdir;

// Helper to get directory size
fn get_dir_size(path: &Path) -> io::Result<u64> {
    let mut total_size = 0;
    for entry in WalkDir::new(path) {
        total_size += entry?.metadata()?.len();
    }
    Ok(total_size)
}

// Helper to format bytes into a readable string
fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;

    if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

struct BenchResult {
    profile: String,
    archiver: String,
    archive_size: u64,
    create_time: Duration,
    extract_time: Duration,
}

struct BenchProfile {
    name: String,
    level: i32,
    mfa_threads: Option<String>,
    mfa_workers: Option<String>,
    zstd_threads: bool,
}

fn main() -> io::Result<()> {
    let source_data_path = PathBuf::from("/Users/oleksandr/Desktop/Development/BTSL/DATASET");

    let profiles = vec![
        BenchProfile {
            name: "L7 Single-Thread".to_string(),
            level: 7,
            mfa_threads: None,
            mfa_workers: None,
            zstd_threads: false,
        },
        BenchProfile {
            name: "L7 MFA --threads".to_string(),
            level: 7,
            mfa_threads: Some("0".to_string()),
            mfa_workers: None,
            zstd_threads: true,
        },
        BenchProfile {
            name: "L7 MFA --workers".to_string(),
            level: 7,
            mfa_threads: None,
            mfa_workers: Some("auto".to_string()),
            zstd_threads: true,
        },
    ];

    let mut results: Vec<BenchResult> = Vec::new();

    println!("--- Starting Benchmark ---");
    println!("Source dataset: {}", source_data_path.display());

    for profile in &profiles {
        println!("\n--- Benchmarking Profile: {} ---", profile.name);

        // --- MFA Archiver Benchmark ---
        let temp_dir_mfa = tempdir()?;
        let archive_path_mfa = temp_dir_mfa.path().join("test.mfa");

        let start_create_mfa = Instant::now();
        let mut cmd_create_mfa = Command::new("target/release/aatrnnbdye");
        cmd_create_mfa.arg("create")
            .arg("--level").arg(profile.level.to_string())
            .arg("-o").arg(&archive_path_mfa)
            .arg(&source_data_path);

        if let Some(threads) = &profile.mfa_threads {
            cmd_create_mfa.arg("--threads").arg(threads);
        }
        if let Some(workers) = &profile.mfa_workers {
            cmd_create_mfa.arg("--workers").arg(workers);
        }

        let status_create_mfa = cmd_create_mfa.status()?;
        let duration_create_mfa = start_create_mfa.elapsed();
        if !status_create_mfa.success() {
            eprintln!("MFA archiver creation failed for profile '{}'", profile.name);
            continue;
        }

        let extract_dir_mfa = temp_dir_mfa.path().join("extracted_mfa");
        fs::create_dir_all(&extract_dir_mfa)?;
        let start_extract_mfa = Instant::now();
        Command::new("target/release/aatrnnbdye")
            .arg("extract")
            .arg(&archive_path_mfa)
            .arg("-o")
            .arg(&extract_dir_mfa)
            .status()?;
        let duration_extract_mfa = start_extract_mfa.elapsed();
        results.push(BenchResult {
            profile: profile.name.clone(),
            archiver: "MFA".to_string(),
            archive_size: fs::metadata(&archive_path_mfa)?.len(),
            create_time: duration_create_mfa,
            extract_time: duration_extract_mfa,
        });

        // --- tar + zstd Benchmark ---
        let temp_dir_tar = tempdir()?;
        let archive_path_tar = temp_dir_tar.path().join("test.tar.zst");

        let start_create_tar = Instant::now();
        let tar_process = Command::new("tar")
            .arg("-c")
            .arg("-C").arg(source_data_path.parent().unwrap())
            .arg(source_data_path.file_name().unwrap())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let mut cmd_zstd_create = Command::new("zstd");
        cmd_zstd_create.arg(format!("-{}", profile.level))
            .arg("-o").arg(&archive_path_tar);
        if profile.zstd_threads {
            cmd_zstd_create.arg("-T0");
        }
        cmd_zstd_create.stdin(tar_process.stdout.unwrap()).status()?;
        let duration_create_tar = start_create_tar.elapsed();

        let extract_dir_tar = temp_dir_tar.path().join("extracted_tar");
        fs::create_dir_all(&extract_dir_tar)?;
        let start_extract_tar = Instant::now();
        let zstd_extract_process = Command::new("zstd")
            .arg("-d").arg("-c").arg(&archive_path_tar)
            .stdout(std::process::Stdio::piped()).spawn()?;

        Command::new("tar")
            .arg("-x")
            .arg("-C").arg(&extract_dir_tar)
            .stdin(zstd_extract_process.stdout.unwrap()).status()?;
        let duration_extract_tar = start_extract_tar.elapsed();
        results.push(BenchResult {
            profile: profile.name.clone(),
            archiver: "tar+zstd".to_string(),
            archive_size: fs::metadata(&archive_path_tar)?.len(),
            create_time: duration_create_tar,
            extract_time: duration_extract_tar,
        });
    }

    // --- Results ---
    println!("\n--- Benchmark Results ---");
    let original_size = get_dir_size(&source_data_path)?;
    println!("Original Size: {}", format_bytes(original_size));
    println!("\n| Profile            | Archiver   | Archive Size | Ratio   | Create Time | Extract Time |");
    println!("| :---               | :---       | :---         | :---    | :---        | :---         |");

    for result in results {
        println!(
            "| {:<18} | {:<10} | {:<12} | {:<7.2}% | {:<11.2?} | {:<12.2?} |",
            result.profile,
            result.archiver,
            format_bytes(result.archive_size),
            (result.archive_size as f64 / original_size as f64) * 100.0,
            result.create_time,
            result.extract_time
        );
    }


    Ok(())
}
