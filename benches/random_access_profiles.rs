//! Random-access benchmark for multiple MFA modes.
//!
//! Builds archives with different creation profiles (base, threads, katana, seekable)
//! then measures average wall-time to extract N random files.
//!
//! Run with:
//!     cargo bench --bench random_access_profiles -- --dataset-path <PATH> --files 10
//!

use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;


use rand::seq::SliceRandom;
use rand::thread_rng;
use walkdir::WalkDir;

struct Args {
    dataset_path: String,
    files: usize,
    rebuild: bool,
}

#[derive(Clone, Copy)]
enum Tool {
    Mfa,
    TarZstd,
    SevenZ,
}

struct Profile {
    name: &'static str,
    tool: Tool,
    create_args: Vec<&'static str>, // for MFA only
    tar_level: Option<&'static str>, // for tar+zstd
    sevenz_level: Option<&'static str>, // for 7z
}

fn get_mfa_bin() -> Result<PathBuf, Box<dyn Error>> {
    let p = PathBuf::from("target/release/aatrnnbdye");
    if !p.exists() {
        return Err("Binary not built – run `cargo build --release`".into());
    }
    Ok(p)
}

fn run_and_time(mut cmd: Command) -> Result<f64, Box<dyn Error>> {
    let start = Instant::now();
    let status = cmd.stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    if !status.success() {
        return Err(format!("Command failed: {:?}", cmd).into());
    }
    Ok(start.elapsed().as_secs_f64() * 1000.0) // ms
}

fn build_archive(bin: &Path, dataset: &str, out: &Path, profile: &Profile, rebuild: bool) -> Result<(), Box<dyn Error>> {
    if out.exists() && !rebuild {
        return Ok(());
    }
    fs::create_dir_all(out.parent().unwrap())?;
    if out.exists() {
        fs::remove_file(out)?;
    }

    match profile.tool {
        Tool::Mfa => {
            let mut cmd = Command::new(bin);
            cmd.args(&profile.create_args)
                .arg("--output")
                .arg(out)
                .arg(dataset);
            println!("Creating {:?} …", out.file_name().unwrap());
            run_and_time(cmd)?;
        }
        Tool::TarZstd => {
            println!("Creating tar+zstd … {:?}", out.file_name().unwrap());
            // tar writes to stdout, pipe through zstd
            // tar -C dataset -cf - . | zstd -3 -T0 -o out
            let status = Command::new("bash")
                .args(["-c", &format!(
                    "tar -C '{}' -cf - . | zstd -{} -T0 -o '{}'",
                    dataset,
                    profile.tar_level.unwrap_or("3"),
                    out.to_string_lossy()
                )])
                .status()?;
            if !status.success() {
                return Err("tar+zstd build failed".into());
            }
        }
        Tool::SevenZ => {
            println!("Creating 7z … {:?}", out.file_name().unwrap());
            let status = Command::new("7z")
                .args([
                    "a",
                    "-t7z",
                    "-m0=lzma2",
                    &format!("-mx={}", profile.sevenz_level.unwrap_or("7")),
                    out.to_str().unwrap(),
                    &format!("{}/.", dataset),
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            if !status.success() {
                return Err("7z build failed".into());
            }
        }
    }
    Ok(())
}

fn collect_files(dataset: &str) -> Vec<PathBuf> {
    WalkDir::new(dataset)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(dataset).unwrap().to_path_buf())
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut dataset_path = "/Users/oleksandr/Desktop/Development/BTSL/DATASET".to_string();
    let mut files = 5usize;
    let mut rebuild = false;
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--dataset-path" => {
                if let Some(val) = iter.next() {
                    dataset_path = val;
                }
            }
            "--files" => {
                if let Some(val) = iter.next() {
                    files = val.parse().unwrap_or(5);
                }
            }
            "--rebuild" => rebuild = true,
            other => {
                // ignore unknown flags coming from cargo bench harness
                if other.starts_with("--") {
                    if iter.next().is_some() { /* skip potential value */ }
                }
            }
        }
    }
    let args = Args { dataset_path, files, rebuild };
    let dataset = &args.dataset_path;

    // Profiles definitions
    let profiles = vec![
        // Katana levels
        Profile { name: "K1_auto", tool: Tool::Mfa, create_args: vec!["create", "--katana", "--level", "1"], tar_level: None, sevenz_level: None },
        Profile { name: "K1_adapt", tool: Tool::Mfa, create_args: vec!["create", "--katana", "--level", "1", "--adaptive"], tar_level: None, sevenz_level: None },
        Profile { name: "K3_auto", tool: Tool::Mfa, create_args: vec!["create", "--katana", "--level", "3"], tar_level: None, sevenz_level: None },
        Profile { name: "K3_adapt", tool: Tool::Mfa, create_args: vec!["create", "--katana", "--level", "3", "--adaptive"], tar_level: None, sevenz_level: None },
        Profile { name: "K7_auto", tool: Tool::Mfa, create_args: vec!["create", "--katana", "--level", "7"], tar_level: None, sevenz_level: None },
        Profile { name: "K7_adapt", tool: Tool::Mfa, create_args: vec!["create", "--katana", "--level", "7", "--adaptive"], tar_level: None, sevenz_level: None },
        // External formats
        Profile { name: "tar_zstd", tool: Tool::TarZstd, create_args: vec![], tar_level: Some("3"), sevenz_level: None },
        Profile { name: "7z_lzma2", tool: Tool::SevenZ, create_args: vec![], tar_level: None, sevenz_level: Some("7") },
    ];

    let bin = get_mfa_bin()?;
    let archive_dir = PathBuf::from("benches/test_archives_random");

    // Build archives
    for p in &profiles {
        let out = match p.tool {
            Tool::Mfa => archive_dir.join(format!("dataset_{}.mfa", p.name.to_lowercase())),
            Tool::TarZstd => archive_dir.join(format!("dataset_{}.tar.zst", p.name.to_lowercase())),
            Tool::SevenZ => archive_dir.join(format!("dataset_{}.7z", p.name.to_lowercase())),
        };
        build_archive(&bin, dataset, &out, p, args.rebuild)?;
    }

    // Prepare random files
    let all_files = collect_files(dataset);
    let mut rng = thread_rng();
    let random_files: Vec<PathBuf> = all_files
        .choose_multiple(&mut rng, args.files)
        .cloned()
        .collect();
    if random_files.is_empty() {
        println!("Dataset empty");
        return Ok(());
    }

    println!("\nBenchmark: random access extraction of {} files", random_files.len());
    println!("| {:<10} | {:>12} |", "Profile", "Avg Wall-ms");
    println!("|{}|{}|", "-".repeat(11), "-".repeat(13));

    let mut extract_dir = archive_dir.join("extract_tmp");

    for p in &profiles {
        let archive_path = match p.tool {
            Tool::Mfa => archive_dir.join(format!("dataset_{}.mfa", p.name.to_lowercase())),
            Tool::TarZstd => archive_dir.join(format!("dataset_{}.tar.zst", p.name.to_lowercase())),
            Tool::SevenZ => archive_dir.join(format!("dataset_{}.7z", p.name.to_lowercase())),
        };
        fs::create_dir_all(&extract_dir)?;
        let mut wall_sum_ms = 0.0;

        for f in &random_files {
            match p.tool {
                Tool::Mfa => {
                    let mut cmd = Command::new(&bin);
                    cmd.arg("extract");
                    cmd.arg("--output");
                    cmd.arg(&extract_dir);
                    cmd.arg(&archive_path);
                    cmd.arg(f);
                    wall_sum_ms += run_and_time(cmd)?;
                }
                Tool::TarZstd => {
                    let rel_path = format!("./{}", f.to_string_lossy());
                    let cmd_str = format!(
                        "zstd -d -c '{}' | tar -xf - -C '{}' '{}'",
                        archive_path.to_string_lossy(),
                        extract_dir.to_string_lossy(),
                        rel_path
                    );
                    let mut cmd = Command::new("bash");
                    cmd.args(["-c", &cmd_str]);
                    wall_sum_ms += run_and_time(cmd)?;
                }
                Tool::SevenZ => {
                    let rel = f.to_string_lossy();
                    let mut cmd = Command::new("7z");
                    cmd.args([
                        "x",
                        archive_path.to_str().unwrap(),
                        &rel,
                        &format!("-o{}", extract_dir.to_string_lossy()),
                        "-y",
                    ]);
                    wall_sum_ms += run_and_time(cmd)?;
                }
            }
            fs::remove_dir_all(&extract_dir)?; // clean between extractions
            fs::create_dir_all(&extract_dir)?;
        }
        fs::remove_dir_all(&extract_dir)?;
        extract_dir = archive_dir.join("extract_tmp");

        let avg_ms = wall_sum_ms / random_files.len() as f64;
        println!("| {:<12} | {:>12.3} |", p.name, avg_ms);
    }

    Ok(())
}
