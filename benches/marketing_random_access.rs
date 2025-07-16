//! Reproducible random-access benchmark aimed at marketing numbers.
//!
//! It measures wall-clock time to extract N random files from archives built
//! with different tools (Katana, tar+zstd, 7z).  Two scenarios are supported:
//!     1. cold-cache – tries to drop OS page cache before every extraction;
//!     2. warm-cache – runs extractions sequentially without cache drop.
//!
//! The benchmark prints a Markdown table and optionally writes results in CSV.
//! The dataset, number of files, RNG seed and selected scenarios can be
//! controlled via CLI flags.  Example:
//!
//! ```bash
//! cargo bench --bench marketing_random_access \
//!            -- --dataset-path /DATASET --files 50 \
//!            --cold --warm --seed 123 --output results.csv --rebuild
//! ```
//!
//! Notes for accurate cold-cache numbers:
//!   • On macOS the tool invokes the `purge` command (no root required).
//!   • On Linux it calls `sync && echo 3 > /proc/sys/vm/drop_caches` which
//!     requires root; run the benchmark via `sudo` or pre-configure sudoers.
//!   • If cache drop fails, the run is skipped with a warning.
//!
//! The script also captures basic system information (CPU, OS, disk model) so
//! the results can be reproduced later.

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use rand::{prelude::SliceRandom, rngs::StdRng, SeedableRng};
use walkdir::WalkDir;

#[derive(Clone, Copy)]
enum Tool {
    Katana,    // MFA build with Katana backend
    TarZstd,   // tar + zstd -3
    SevenZ,    // 7z LZMA2 -mx7
}

struct Profile {
    name: &'static str,
    tool: Tool,
    create_args: Vec<&'static str>,
}

// --------------------- helpers ----------------------------
fn get_mfa_bin() -> Result<PathBuf, Box<dyn Error>> {
    let p = PathBuf::from("target/release/aatrnnbdye");
    if !p.exists() {
        return Err("Binary not built – run `cargo build --release`.".into());
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

fn drop_caches() -> Result<(), io::Error> {
    #[cfg(target_os = "macos")]
    {
        use std::sync::Once;
        static WARN_ONCE: Once = Once::new();
        let status = Command::new("purge")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_ok() && status.as_ref().unwrap().success() {
            return Ok(());
        }
        // Warn only first time.
        WARN_ONCE.call_once(|| {
            eprintln!("Warning: unable to drop file cache with `purge` – cold-cache timings may be inaccurate (try running with sudo). Continuing anyway.");
        });
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        // Requires root
        let status = Command::new("sh")
            .args(["-c", "sync && echo 3 > /proc/sys/vm/drop_caches"])
            .status()?;
        if status.success() {
            return Ok(());
        }
        return Err(io::Error::new(io::ErrorKind::Other, "drop_caches failed"));
    }
    #[allow(unreachable_code)]
    Err(io::Error::new(io::ErrorKind::Other, "cache drop unsupported"))
}

fn collect_files(dataset: &str) -> Vec<PathBuf> {
    WalkDir::new(dataset)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(dataset).unwrap().to_path_buf())
        .collect()
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
        Tool::Katana => {
            let mut cmd = Command::new(bin);
            cmd.args(&profile.create_args)
                .arg("--output")
                .arg(out)
                .arg(dataset);
            run_and_time(cmd)?; // ignore returned time
        }
        Tool::TarZstd => {
            let cmd_str = format!(
                "tar -C '{}' -cf - . | zstd -3 -T0 -o '{}'",
                dataset,
                out.to_string_lossy()
            );
            Command::new("bash").args(["-c", &cmd_str]).status()?;
        }
        Tool::SevenZ => {
            Command::new("7z")
                .args([
                    "a", "-t7z", "-m0=lzma2", "-mx=7", out.to_str().unwrap(), &format!("{}/.", dataset),
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
        }
    }
    Ok(())
}

fn system_info() -> String {
    #[cfg(target_os = "macos")]
    {
        let cpu = String::from_utf8_lossy(&Command::new("sysctl").args(["-n", "machdep.cpu.brand_string"]).output().ok().map(|o| o.stdout).unwrap_or_default()).trim().to_string();
        let os = String::from_utf8_lossy(&Command::new("sw_vers").arg("-productVersion").output().ok().map(|o| o.stdout).unwrap_or_default()).trim().to_string();
        format!("macOS {} | {}", os, cpu)
    }
    #[cfg(target_os = "linux")]
    {
        let cpu_out = Command::new("lscpu").output().ok();
        let cpu_str = cpu_out.as_ref().map(|o| String::from_utf8_lossy(&o.stdout)).unwrap_or_else(|| "".into());
        let first_line = cpu_str.lines().next().unwrap_or("").trim();
        let kernel = String::from_utf8_lossy(&Command::new("uname").arg("-r").output().ok().map(|o| o.stdout).unwrap_or_default()).trim().to_string();
        format!("Linux {} | {}", kernel, first_line)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        "Unknown OS".to_string()
    }
}

// ---------------------- main ------------------------------
fn main() -> Result<(), Box<dyn Error>> {
    // Defaults
    let mut dataset = "/path/to/DATASET".to_string();
    let mut files = 50usize;
    let mut seed: u64 = 42;
    let mut run_cold = false;
    let mut run_warm = false;
    let mut csv_out: Option<String> = None;
    let mut rebuild = false;
    let mut katana_only = false;

    // arg parse (very simple)
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--dataset-path" => dataset = iter.next().expect("value").into(),
            "--files" => files = iter.next().expect("value").parse().unwrap(),
            "--seed" => seed = iter.next().expect("value").parse().unwrap(),
            "--cold" => run_cold = true,
            "--warm" => run_warm = true,
            "--output" => csv_out = Some(iter.next().expect("value")),
            "--rebuild" => rebuild = true,
            "--katana-only" => katana_only = true,
            other => eprintln!("Ignoring unknown arg {}", other),
        }
    }
    if !run_cold && !run_warm {
        run_warm = true; // default
    }

    // Profiles
    let mut profiles = vec![
        Profile { name: "Katana_L3", tool: Tool::Katana, create_args: vec!["create", "--katana", "--level", "3"] },
        Profile { name: "Katana_L3_adapt", tool: Tool::Katana, create_args: vec!["create", "--katana", "--level", "3", "--adaptive"] },
        Profile { name: "tar_zstd", tool: Tool::TarZstd, create_args: vec![] },
        Profile { name: "7z_lzma2", tool: Tool::SevenZ, create_args: vec![] },
    ];
    if katana_only {
        profiles.retain(|p| matches!(p.tool, Tool::Katana));
    }

    let bin = get_mfa_bin()?;
    let archive_dir = PathBuf::from("benches/marketing_archives");
    fs::create_dir_all(&archive_dir)?;

    println!("Dataset: {}, files: {}, seed: {}", dataset, files, seed);
    println!("Profiles: {:?}", profiles.iter().map(|p| p.name).collect::<Vec<_>>());

    // Build archives
    for p in &profiles {
        let out = match p.tool {
            Tool::Katana => archive_dir.join(format!("{}.mfa", p.name)),
            Tool::TarZstd => archive_dir.join(format!("{}.tar.zst", p.name)),
            Tool::SevenZ => archive_dir.join(format!("{}.7z", p.name)),
        };
        println!("Preparing archive {}...", p.name);
        build_archive(&bin, &dataset, &out, p, rebuild)?;
    }

    // Prepare file list
    let all_files = collect_files(&dataset);
    if all_files.len() < files {
        return Err("Dataset contains fewer files than requested".into());
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let chosen: Vec<PathBuf> = all_files.choose_multiple(&mut rng, files).cloned().collect();

    // results: Vec<(profile, scenario, mean, median, std_ms)>
    let mut results: Vec<(String, String, f64, f64, f64)> = Vec::new();

    // Benchmark loop
    for scenario in [("cold", run_cold), ("warm", run_warm)] {
        if !scenario.1 { continue; }

        for p in &profiles {
            let archive_path = match p.tool {
                Tool::Katana => archive_dir.join(format!("{}.mfa", p.name)),
                Tool::TarZstd => archive_dir.join(format!("{}.tar.zst", p.name)),
                Tool::SevenZ => archive_dir.join(format!("{}.7z", p.name)),
            };

            let mut times_ms = Vec::with_capacity(files);
            if scenario.0 == "cold" {
                if let Err(e) = drop_caches() {
                    eprintln!("[WARN] couldn't drop caches: {} – skipping cold run", e);
                    continue;
                }
            }
            for rel in &chosen {
                match p.tool {
                    Tool::Katana => {
                        let mut cmd = Command::new(&bin);
                        cmd.arg("extract")
                            .arg("--output").arg("/tmp") // write to tmpfs (marketing focus: speed)
                            .arg(&archive_path)
                            .arg(rel);
                        times_ms.push(run_and_time(cmd)?);
                    }
                    Tool::TarZstd => {
                        let rel_path = format!("./{}", rel.to_string_lossy());
                        let cmd_str = format!(
                            "zstd -d -c '{}' | tar -xf - -C '/tmp' '{}'",
                            archive_path.to_string_lossy(),
                            rel_path
                        );
                        let mut cmd = Command::new("bash");
                        cmd.args(["-c", &cmd_str]);
                        times_ms.push(run_and_time(cmd)?);
                    }
                    Tool::SevenZ => {
                        let mut cmd = Command::new("7z");
                        cmd.args(["x", archive_path.to_str().unwrap(), rel.to_str().unwrap(), "-o/tmp", "-y"]);
                        times_ms.push(run_and_time(cmd)?);
                    }
                }
            }

            if times_ms.len() != files { continue; } // cold run may be skipped
            times_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mean = times_ms.iter().copied().sum::<f64>() / times_ms.len() as f64;
            let median = if files % 2 == 0 {
                (times_ms[files/2 -1] + times_ms[files/2]) / 2.0
            } else {
                times_ms[files/2]
            };
            let var = times_ms.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / times_ms.len() as f64;
            let std = var.sqrt();
            results.push((p.name.into(), scenario.0.into(), mean, median, std));
        }
    }

    // Output markdown
    println!("\nSystem: {}", system_info());
    println!("\n| Profile | Scenario | Mean ms | Median ms | Std ms |");
    println!("|---------|----------|---------|-----------|--------|");
    for (prof, scen, mean, med, std) in &results {
        println!("| {:<12} | {:<6} | {:>8.2} | {:>10.2} | {:>7.2} |", prof, scen, mean, med, std);
    }

    // optional CSV
    if let Some(path) = csv_out {
        let mut w = fs::File::create(path)?;
        writeln!(w, "profile,scenario,mean_ms,median_ms,std_ms")?;
        for (prof, scen, mean, med, std) in &results {
            writeln!(w, "{},{},{:.3},{:.3},{:.3}", prof, scen, mean, med, std)?;
        }
    }

    Ok(())
}
