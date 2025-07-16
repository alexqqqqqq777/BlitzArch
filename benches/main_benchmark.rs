// benches/main_benchmark.rs
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DATASET_PATH: &str = "/Users/oleksandr/Desktop/Development/BTSL/DATASET";
const ARCHIVER_BIN: &str = "./target/release/aatrnnbdye";

struct BenchProfile {
    name: String,
    archiver: Archiver,
    level: i32,
    workers: bool,
    preprocess: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Archiver {
    Mfa,
    TarZstd,
}

struct BenchResult {
    profile_name: String,
    archiver_name: String,
    archive_size: u64,
    create_time: Duration,
    create_mem: u64,
    extract_time: Duration,
    extract_mem: u64,
}

fn main() -> io::Result<()> {
    println!("--- Starting Final Benchmark ---");
    io::stdout().flush()?;

    println!("Building latest version of the archiver...");
    io::stdout().flush()?;
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()?;
    if !build_status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to build archiver",
        ));
    }

    let dataset_path = Path::new(DATASET_PATH);
    println!("Source dataset: {}\n", dataset_path.display());
    io::stdout().flush()?;

    let profiles: Vec<BenchProfile> = vec![
        BenchProfile { name: "MFA (L3, workers, preproc)".to_string(), archiver: Archiver::Mfa, level: 3, workers: true, preprocess: true },
        BenchProfile { name: "MFA (L7, workers, preproc)".to_string(), archiver: Archiver::Mfa, level: 7, workers: true, preprocess: true },
        BenchProfile { name: "MFA (L12, workers, preproc)".to_string(), archiver: Archiver::Mfa, level: 12, workers: true, preprocess: true },
        BenchProfile { name: "tar+zstd (L3, workers)".to_string(), archiver: Archiver::TarZstd, level: 3, workers: true, preprocess: false },
        BenchProfile { name: "tar+zstd (L7, workers)".to_string(), archiver: Archiver::TarZstd, level: 7, workers: true, preprocess: false },
        BenchProfile { name: "tar+zstd (L12, workers)".to_string(), archiver: Archiver::TarZstd, level: 12, workers: true, preprocess: false },
    ];

    let mut results = Vec::new();

    for profile in &profiles {
        println!("--- Benchmarking Profile: {} ---", profile.name);
        io::stdout().flush()?;
        let result = run_benchmark(profile, dataset_path)?;
        results.push(result);
    }

    print_results_table(&results, dataset_path)?;

    Ok(())
}

fn run_benchmark(profile: &BenchProfile, dataset_path: &Path) -> io::Result<BenchResult> {
    let temp_dir = tempfile::tempdir()?;
    let archive_path = temp_dir.path().join(match profile.archiver {
        Archiver::Mfa => "test.mfa",
        Archiver::TarZstd => "test.tar.zst",
    });
    let extract_path = temp_dir.path().join("extracted");
    fs::create_dir(&extract_path)?;

    // --- Create phase ---
    println!("Creating archive...");
    let (create_time, create_mem, archive_size) = match profile.archiver {
        Archiver::Mfa => {
            let mut cmd = Command::new(ARCHIVER_BIN);
            cmd.args(["create", dataset_path.to_str().unwrap(), "-o"]).arg(&archive_path);
            cmd.arg("--level").arg(profile.level.to_string());
            if profile.workers {
                cmd.arg("--workers").arg("auto");
            }
            if profile.preprocess {
                cmd.arg("--preprocess");
            }
            let (t, m) = measure_command(cmd)?;
            let sz = fs::metadata(&archive_path)?.len();
            (t, m, sz)
        }
        Archiver::TarZstd => {
            let tar_cmd = format!("tar -cf - -C {} .", dataset_path.display());
            let zstd_cmd = format!("zstd -T0 --no-check --long=31 -{} -o {}", profile.level, archive_path.display());
            let pipeline = format!("set -o pipefail; {} | {}", tar_cmd, zstd_cmd);
            let mut shell_cmd = Command::new("bash");
            shell_cmd.arg("-c").arg(&pipeline);
            let (t, m) = measure_command(shell_cmd)?;
            let sz = fs::metadata(&archive_path)?.len();
            (t, m, sz)
        }
    };

    // --- Extract phase ---
    println!("Extracting archive...");
    let (extract_time, extract_mem) = match profile.archiver {
        Archiver::Mfa => {
            let mut cmd = Command::new(ARCHIVER_BIN);
            cmd.args(["extract", archive_path.to_str().unwrap(), "-o"]).arg(&extract_path);
            measure_command(cmd)?
        }
        Archiver::TarZstd => {
            let tar_cmd = format!("tar -xf - -C {}", extract_path.display());
            let zstd_cmd = format!("zstd -d -T0 --no-check --long=31 -c {}", archive_path.display());
            let pipeline = format!("set -o pipefail; {} | {}", zstd_cmd, tar_cmd);
            let mut shell_cmd = Command::new("bash");
            shell_cmd.arg("-c").arg(&pipeline);
            measure_command(shell_cmd)?
        }
    };

    Ok(BenchResult {
        profile_name: profile.name.to_string(),
        archiver_name: match profile.archiver {
            Archiver::Mfa => "MFA",
            Archiver::TarZstd => "tar.zst",
        }.to_string(),
        archive_size,
        create_time,
        create_mem,
        extract_time,
        extract_mem,
    })
}

fn measure_command(mut command: Command) -> io::Result<(Duration, u64)> {
    let time_path = "/usr/bin/time";
    let mut full_command = Command::new(time_path);
    full_command.arg("-l");

    if matches!(command.get_program().to_str(), Some("sh") | Some("bash")) {
        full_command.arg("bash").arg("-c").arg(command.get_args().last().unwrap());
    } else {
        full_command.arg(command.get_program());
        full_command.args(command.get_args());
    }

    let start = Instant::now();
    let output = full_command.stderr(Stdio::piped()).output()?;
    let duration = start.elapsed();

    if !output.status.success() {
        eprintln!("Command failed:");
        eprintln!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Err(io::Error::new(io::ErrorKind::Other, "Command execution failed"));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let peak_mem = parse_peak_mem(&stderr).unwrap_or(0);

    Ok((duration, peak_mem))
}

fn parse_peak_mem(stderr: &str) -> Option<u64> {
    stderr
        .lines()
        .find(|line| line.contains("maximum resident set size"))
        .and_then(|line| line.split_whitespace().next())
        .and_then(|val| val.parse::<u64>().ok())
}

fn get_dir_size(path: &Path) -> io::Result<u64> {
    let mut total_size = 0;
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let metadata = entry.metadata().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        if metadata.is_file() {
            total_size += metadata.len();
        }
    }
    Ok(total_size)
}

fn print_results_table(results: &[BenchResult], dataset_path: &Path) -> io::Result<()> {
    let original_size = get_dir_size(dataset_path)?;
    println!("\n--- Benchmark Results ---");
    println!("Original Size: {:.2} MiB\n", original_size as f64 / 1024.0 / 1024.0);

    let mut writer = io::stdout();
    writeln!(
        writer,
        "| {:<24} | {:<12} | {:<10} | {:<11} | {:<15} | {:<12} | {:<16} |",
        "Profile", "Archive Size", "Ratio", "Create Time", "Create Peak Mem", "Extract Time", "Extract Peak Mem"
    )?;
    writeln!(
        writer,
        "| :---                       | :---         | :---    | :---        | :---            | :---         | :---             |"
    )?;

    for r in results {
        let ratio = original_size as f64 / r.archive_size as f64; // compression factor
let ratio_str = format!("{:.2}×", ratio);
        writeln!(
            writer,
            "| {:<24} | {:<12} | {:<10} | {:<11.2?} | {:<15.2} | {:<12.2?} | {:<16.2} |",
            r.profile_name,
            format!("{:.2} MiB", r.archive_size as f64 / 1024.0 / 1024.0),
            &ratio_str,
            r.create_time,
            format!("{:.2} MiB", r.create_mem as f64 / 1024.0 / 1024.0),
            r.extract_time,
            format!("{:.2} MiB", r.extract_mem as f64 / 1024.0 / 1024.0)
        )?;
    }

    Ok(())
}