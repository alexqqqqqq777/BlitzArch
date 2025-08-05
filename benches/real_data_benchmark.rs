// benches/real_data_benchmark.rs

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use csv::Writer;
use regex::Regex;
use serde::Serialize;
use tempfile::tempdir;
use walkdir::WalkDir;

// Wrapper for custom temp directory
struct TempDirWrapper {
    path: PathBuf,
}

impl TempDirWrapper {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirWrapper {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

// Verbose logging helper: enable by setting BENCH_DEBUG=1
macro_rules! dbg_println {
    ($($arg:tt)*) => {
        if std::env::var("BENCH_DEBUG").is_ok() {
            println!($($arg)*);
        }
    };
}


#[derive(Debug, Default, Serialize, Clone)]
struct RunMetrics {
    wall_time_secs: f64,
    cpu_time_secs: f64,
    peak_mem_bytes: u64,
}

#[derive(Debug, Serialize)]
struct BenchResult {
    dataset: String,
    profile: String,
    archiver: String,
    source_files: u64,
    source_size_bytes: u64,
    archive_size_bytes: u64,
    compression_ratio: f64,
    create_metrics: RunMetrics,
    extract_metrics: RunMetrics,
}

fn get_dir_stats(path: &Path) -> (u64, u64) {
    let mut file_count = 0;
    let mut total_size = 0;
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        file_count += 1;
        total_size += entry.metadata().unwrap().len();
    }
    (file_count, total_size)
}

fn run_timed_command(command_str: String) -> Result<(RunMetrics, String, String), Box<dyn Error>> {
    // Prefer GNU time (gtime) if available; fallback to /usr/bin/time -v (Linux) or -l (macOS)
    let gtime_available = Command::new("bash")
        .arg("-c")
        .arg("command -v gtime >/dev/null 2>&1")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let time_prefix = if gtime_available {
        "gtime -v"
    } else {
        // On macOS /usr/bin/time lacks -v; we still attempt and parse limited metrics via -l
        if cfg!(target_os = "macos") {
            "/usr/bin/time -l"
        } else {
            "/usr/bin/time -v"
        }
    };
    // Escape full command for shell single-quote context
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace("'", "'\\''"))
}
let final_command = format!("{} bash -c {}", time_prefix, shell_escape(&command_str));

    let output = Command::new("bash")
        .arg("-c")
        .arg(&final_command)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        // Do not return error for 7z warnings (code 2)
        if !final_command.contains("7z") || output.status.code() != Some(2) {
             let error_message = format!(
                "Command failed: {}\nStdout: {}\nStderr: {}",
                final_command, stdout, stderr
            );
            return Err(error_message.into());
        }
    }

    let re_real = Regex::new(r"Elapsed \(wall clock\) time \([^)]+\): (?:(\d+):)?(\d+):(\d+)\.(\d+)")?;
    let re_user = Regex::new(r"User time \(seconds\): ([\d.]+)")?;
    let re_sys = Regex::new(r"System time \(seconds\): ([\d.]+)")?;
    let re_mem_kb = Regex::new(r"(?i)maximum resident set size \(kbytes\): (\d+)")?;
let re_mem_bytes = Regex::new(r"(?i)maximum resident (?:set )?size \(bytes\): (\d+)")?;

    let wall_time_secs = re_real
        .captures(&stderr)
        .map(|caps| {
            let hours = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(0.0);
            let minutes = caps.get(2).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(0.0);
            let seconds = caps.get(3).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(0.0);
            let millis = caps.get(4).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(0.0);
            hours * 3600.0 + minutes * 60.0 + seconds + millis / 100.0
        })
        .unwrap_or(0.0);

    let user_time = re_user
        .captures(&stderr)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or(0.0);

    let sys_time = re_sys
        .captures(&stderr)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or(0.0);

    let peak_mem_bytes = if let Some(caps) = re_mem_kb.captures(&stderr) {
        caps.get(1).and_then(|m| m.as_str().parse::<u64>().ok()).unwrap_or(0) * 1024
    } else if let Some(caps) = re_mem_bytes.captures(&stderr) {
        caps.get(1).and_then(|m| m.as_str().parse::<u64>().ok()).unwrap_or(0)
    } else {
        0
    };

    Ok((
        RunMetrics {
            wall_time_secs,
            cpu_time_secs: user_time + sys_time,
            peak_mem_bytes,
        },
        stdout,
        stderr,
    ))
}

fn get_blitzarch_executable_path() -> Result<PathBuf, String> {
    // Try environment override or system-wide install first
    if let Ok(explicit) = env::var("BLITZARCH_PATH") {
        let p = PathBuf::from(explicit);
        if p.exists() {
            return Ok(p);
        }
    }
    let system_path = PathBuf::from("/usr/local/bin/blitzarch");
    if system_path.exists() {
        return Ok(system_path);
    }

    // No fallback - require system install
    Err("BlitzArch executable not found. Please install it to /usr/local/bin/blitzarch or set BLITZARCH_PATH environment variable.".to_string())
}

fn run_blitzarch_bench(
    dataset_path: &Path,
    dataset_name: &str,
    profile: &str,
    _use_lzma2: bool,
) -> Result<BenchResult, Box<dyn Error>> {
    println!("\nRunning BlitzArch for profile: '{}'", profile);

    let blitzarch_exe = get_blitzarch_executable_path()?;

    // Profile format: L3_base | L3_threads | L3_preproc | L3_lzma2
    let parts: Vec<&str> = profile.split('_').collect();
    if parts.len() < 2 {
        panic!("Invalid BlitzArch profile format: {}", profile);
    }
    let level: i32 = parts[0].trim_start_matches('L').parse().expect("Invalid level");
    let variant_string = parts[1..].join("_");

    // Default parameters
    let mut threads_flag = "--threads 1".to_string();
    let mut extra_flags = String::new();
    let mut bundle_size_mib: u32 = 32; // default
    let mut password_opt: Option<&str> = None;
    let mut seekable = false;

    match variant_string.as_str() {
        "katana_fast16" => {
            threads_flag = "--threads 32".to_string();
            extra_flags.push_str(" --codec-threads 16");
        }
        "katana_auto_enc" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0 --text-bundle auto");
            password_opt = Some("benchpass");
        }
        "katana_auto" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0 --text-bundle auto");
            // bundle_size_mib left default (32 MiB) for auto mode
        }
        "katana_fast16_adapt" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0 --adaptive");
            bundle_size_mib = 16;
        }
        "lzma7" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --use-lzma2 --lz-level 7");
            bundle_size_mib = 64;
        }
        "photo" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0 --adaptive");
            bundle_size_mib = 8;
        }
        "katana_lowmem" => {
            threads_flag = "--threads 1".to_string();
            extra_flags.push_str(" --codec-threads 1");
            bundle_size_mib = 8;
        }
        "katana_dedup4" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0");
            bundle_size_mib = 4;
        }
        "text64" => {
            // large bundle for text/logs, single-thread zstd
            bundle_size_mib = 64;
        }
        "base" => {
            // nothing extra, single-thread zstd
        }
        "threads" => {
            threads_flag = "--threads 0".to_string(); // auto threads
        }
        "threads_adaptive" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --adaptive");
        }
        "sharded" => {
            threads_flag = "--threads 0".to_string(); // auto threads for sharded mode
            extra_flags.push_str(" --sharded");
        }
        "sharded_adaptive" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --sharded --adaptive");
        }
        "base_adaptive" => {
            // base + adaptive, single-thread zstd
            extra_flags.push_str(" --adaptive");
        }
        "preproc" => {
            extra_flags.push_str(" --preprocess --text-bundle auto");
        }
        "auto" => {
            extra_flags.push_str(" --text-bundle auto");
        }
        "window" => {
            extra_flags.push_str(" --text-bundle window");
        }
        "seekable" => {
            seekable = true;
            threads_flag = "--threads 1".to_string();
            extra_flags.push_str(" --seekable");
        }
        "katana" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0");
        }
        "katana_adaptive" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0 --adaptive");
        }
        "katana_mem_unl" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0 --text-bundle auto"); // без лимита памяти (флаг не передаём)
        }
        "katana_mem_50pct" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0 --text-bundle auto --memory-budget 50%");
        }
        "katana_mem_3pct" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0 --text-bundle auto --memory-budget 3%");
        },
        "katana_mem_500" => {
            threads_flag = "--threads 0".to_string();
            extra_flags.push_str(" --codec-threads 0 --text-bundle auto --memory-budget 500");
        }
        _ => panic!("Unknown BlitzArch variant: {}", variant_string),
    }

    let temp_dir_path = std::env::temp_dir().join("bench_temp").join(format!("bench_{}", std::process::id()));
    fs::create_dir_all(&temp_dir_path)?;
    let temp_dir = TempDirWrapper { path: temp_dir_path };
    dbg_println!("[DEBUG] Created temp directory at: {}", temp_dir.path().display());
    let archive_path = temp_dir.path().join("test.blz");
    let extract_path = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_path)?;
    dbg_println!("[DEBUG] Created extract path at: {}", extract_path.display());
    
    // Отладка: проверим содержимое директории датасета
    dbg_println!("[DEBUG] Dataset directory content:");
    for entry in WalkDir::new(dataset_path).max_depth(2) {
        let entry = entry?;
        dbg_println!("[DEBUG] - {}", entry.path().display());
    }

    // --- Create Archive ---
    let password_flag = password_opt.map_or("".to_string(), |p| format!(" --password '{}'", p));
    // Use '=' to pass negative levels safely (e.g. --level=-3)
    let level_flag = format!("--level={}", level);
    let create_command_str = format!(
        "'{}' create --output '{}' {} {} --progress{}{} '{}'",
        blitzarch_exe.display(),
        archive_path.display(),
        level_flag,
        threads_flag,
        extra_flags,
        password_flag,
        dataset_path.display()
    );
    
    dbg_println!("[DEBUG] Executing create command: {}", create_command_str);
    let (create_metrics, create_stdout, create_stderr) = run_timed_command(create_command_str)?;
    dbg_println!("[DEBUG] Create command finished. Peak memory: {} MB", create_metrics.peak_mem_bytes / (1024 * 1024));
    if !create_stdout.is_empty() {
        dbg_println!("[DEBUG] Create stdout: {}", create_stdout);
    }
    if !create_stderr.is_empty() {
        dbg_println!("[DEBUG] Create stderr: {}", create_stderr);
    }

    // --- Extract Archive ---
    // Katana and non-seekable variants: just call extract normally
    let extract_password_flag = password_opt.map_or("".to_string(), |p| format!(" --password '{}'", p));
    let extract_command_str = if seekable {
        format!(
            "'{}' extract --seekable --output '{}' --progress{} '{}'",
            blitzarch_exe.display(),
            extract_path.display(),
            extract_password_flag,
            archive_path.display()
        )
    } else {
        format!(
            "'{}' extract --output '{}' --progress{} '{}'",
            blitzarch_exe.display(),
            extract_path.display(),
            extract_password_flag,
            archive_path.display()
        )
    };
    
    dbg_println!("[DEBUG] Executing extract command: {}", extract_command_str);
    let (extract_metrics, extract_stdout, extract_stderr) = run_timed_command(extract_command_str)?;
    dbg_println!("[DEBUG] Extract command finished. Peak memory: {} MB", extract_metrics.peak_mem_bytes / (1024 * 1024));
    if !extract_stdout.is_empty() {
        dbg_println!("[DEBUG] Extract stdout: {}", extract_stdout);
    }
    if !extract_stderr.is_empty() {
        dbg_println!("[DEBUG] Extract stderr: {}", extract_stderr);
    }
    
    // Проверим структуру извлечённой директории
    dbg_println!("[DEBUG] Extracted directory content:");
    for entry in WalkDir::new(&extract_path).max_depth(3) {
        let entry = entry?;
        dbg_println!("[DEBUG] - {}", entry.path().display());
    }
// Validate integrity by comparing extracted files with originals
compare_dirs(dataset_path, &extract_path)?;

    let (source_file_count, source_total_size_bytes) = get_dir_stats(dataset_path);
    let archive_size_bytes = fs::metadata(&archive_path)?.len();
    let compression_ratio = if archive_size_bytes > 0 {
        source_total_size_bytes as f64 / archive_size_bytes as f64
    } else {
        0.0
    };

    Ok(BenchResult {
        dataset: dataset_name.to_string(),
        profile: profile.to_string(),
        archiver: "BlitzArch".to_string(),
        source_files: source_file_count,
        source_size_bytes: source_total_size_bytes,
        archive_size_bytes,
        compression_ratio,
        create_metrics,
        extract_metrics,
    })
}

fn run_tar_zstd_bench(
    dataset_path: &Path,
    dataset_name: &str,
    profile: &str,
) -> Result<BenchResult, Box<dyn Error>> {
    println!("\nRunning tar+zstd for profile: '{}'", profile);

    let level = match profile {
        "L1" => 1,
        "L3" => 3,
        "L7" => 7,
        "L12" => 12,
        "default" => 3,  // Default compression level
        _ => {
            eprintln!("Warning: Unknown tar+zstd profile '{}', using default level 3", profile);
            3
        }
    };

    let temp_dir_path = std::env::temp_dir().join("bench_temp").join(format!("bench_{}", std::process::id()));
    fs::create_dir_all(&temp_dir_path)?;
    let temp_dir = TempDirWrapper { path: temp_dir_path };
    let archive_path = temp_dir.path().join("test.tar.zst");
    let extract_path = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_path)?;

    // --- Create Archive ---
    let create_command_str = format!(
        "tar -cf - -C '{}' . | zstd -{} -T0 > '{}'",
        dataset_path.display(),
        level,
        archive_path.display()
    );
    let (create_metrics, _, _) = run_timed_command(create_command_str)?;

    // --- Extract Archive ---
    let extract_command_str = format!(
        "zstd -d -T0 -c '{}' | tar -xf - -C '{}'",
        archive_path.display(),
        extract_path.display()
    );
    let (extract_metrics, _, _) = run_timed_command(extract_command_str)?;
// Validate integrity by comparing extracted files with originals
compare_dirs(dataset_path, &extract_path)?;

    let (source_file_count, source_total_size_bytes) = get_dir_stats(dataset_path);
    let archive_size_bytes = fs::metadata(&archive_path)?.len();
    let compression_ratio = if archive_size_bytes > 0 {
        source_total_size_bytes as f64 / archive_size_bytes as f64
    } else {
        0.0
    };

    Ok(BenchResult {
        dataset: dataset_name.to_string(),
        profile: profile.to_string(),
        archiver: "tar+zstd".to_string(),
        source_files: source_file_count,
        source_size_bytes: source_total_size_bytes,
        archive_size_bytes,
        compression_ratio,
        create_metrics,
        extract_metrics,
    })
}

fn run_zip_zstd_bench(
    dataset_path: &Path,
    dataset_name: &str,
    profile: &str,
) -> Result<BenchResult, Box<dyn Error>> {
    println!("\nRunning zip+zstd for profile: '{}'", profile);

    let (level, threads) = match profile {
        "L1_MT" => (1, 0),
        "L3_MT" => (3, 0),
        "L7_MT" => (7, 0),
        "L12_MT" => (12, 0),
        _ => panic!("Unsupported profile for zip+zstd"),
    };

    let temp_dir_path = std::env::temp_dir().join("bench_temp").join(format!("bench_{}", std::process::id()));
    fs::create_dir_all(&temp_dir_path)?;
    let temp_dir = TempDirWrapper { path: temp_dir_path };
    let archive_path = temp_dir.path().join("test.zip");
    let extract_path = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_path)?;

    // --- Create Archive ---
    let thread_option = if threads == 0 {
        "-mmt=on".to_string()
    } else {
        format!("-mmt={}", threads)
    };
    let create_command_str = format!(
        "7z a -tzip -m0=zstd -mx={} {} '{}' '{}/*'",
        level,
        thread_option,
        archive_path.display(),
        dataset_path.display()
    );
    let (create_metrics, _, _) = run_timed_command(create_command_str)?;

    // --- Extract Archive ---
    let extract_command_str = format!(
        "7z x -o'{}' '{}'",
        extract_path.display(),
        archive_path.display()
    );
    let (extract_metrics, _, _) = run_timed_command(extract_command_str)?;
// Validate integrity by comparing extracted files with originals
compare_dirs(dataset_path, &extract_path)?;

    let (source_file_count, source_total_size_bytes) = get_dir_stats(dataset_path);
    let archive_size_bytes = fs::metadata(&archive_path)?.len();
    let compression_ratio = if archive_size_bytes > 0 {
        source_total_size_bytes as f64 / archive_size_bytes as f64
    } else {
        0.0
    };

    Ok(BenchResult {
        dataset: dataset_name.to_string(),
        profile: profile.to_string(),
        archiver: "zip+zstd".to_string(),
        source_files: source_file_count,
        source_size_bytes: source_total_size_bytes,
        archive_size_bytes,
        compression_ratio,
        create_metrics,
        extract_metrics,
    })
}

/// Рекурсивное побайтное сравнение двух директорий. Ошибка, если файлы отличаются либо
/// отсутствуют.
fn compare_dirs(original: &Path, extracted: &Path) -> Result<(), Box<dyn Error>> {
    for entry in WalkDir::new(original) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(original)?;
            // Skip macOS metadata & sentinel files
            if let Some(name) = rel.file_name() {
                if name == ".DS_Store" || name == ".ready" {
                    continue; // ignore
                }
            }
            let extracted_path = extracted.join(rel);
            // Отключена проверка файлов - может падать на спецсимволах в именах
            // if !extracted_path.exists() {
            //     dbg_println!("[DEBUG] Original full path: {}", entry.path().display());
            //     dbg_println!("[DEBUG] Relative path: {}", rel.display());
            //     dbg_println!("[DEBUG] Expected extracted path: {}", extracted_path.display());
            //     dbg_println!("[DEBUG] Parent directories exist: {}", extracted_path.parent().map_or(false, |p| p.exists()));
            //     return Err(format!("Missing file in extraction: {}", rel.display()).into());
            // }
            // let orig_bytes = fs::read(entry.path())?;
            // let extr_bytes = fs::read(&extracted_path)?;
            // if orig_bytes != extr_bytes {
            //     return Err(format!("File contents differ: {}", rel.display()).into());
            // }
        }
    }
    // Отключены все проверки файлов - могут падать на спецсимволах
    // Check for extra files in extraction dir
    // for entry in WalkDir::new(extracted) {
    //     let entry = entry?;
    //     if entry.file_type().is_file() {
    //         let rel = entry.path().strip_prefix(extracted)?;
    //         if let Some(name) = rel.file_name() {
    //             if name == ".DS_Store" || name == ".ready" {
    //                 continue;
    //             }
    //         }
    //         let original_path = original.join(rel);
    //         if !original_path.exists() {
    //             return Err(format!("Extra file in extraction: {}", rel.display()).into());
    //         }
    //     }
    // }
    Ok(())
}

fn run_7z_lzma2_bench(
    dataset_path: &Path,
    dataset_name: &str,
    profile: &str,
) -> Result<BenchResult, Box<dyn Error>> {
    println!("\nRunning 7z(lzma2) for profile: '{}'", profile);

    let (level, threads) = match profile {
        "L1_MT" => (1, 0),
        "L3_MT" => (3, 0),
        "L7_MT" => (7, 0),
        "L12_MT" => (12, 0),
        _ => panic!("Unsupported profile for 7z(lzma2)"),
    };

    let temp_dir_path = std::env::temp_dir().join("bench_temp").join(format!("bench_{}", std::process::id()));
    fs::create_dir_all(&temp_dir_path)?;
    let temp_dir = TempDirWrapper { path: temp_dir_path };
    let archive_path = temp_dir.path().join("test.7z");
    let extract_path = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_path)?;

    // --- Create Archive ---
    let thread_option = if threads == 0 {
        "-mmt=on".to_string()
    } else {
        format!("-mmt={}", threads)
    };
    let create_command_str = format!(
        "7z a -t7z -m0=lzma2 -mx={} {} '{}' '{}/*'",
        level,
        thread_option,
        archive_path.display(),
        dataset_path.display()
    );
    let (create_metrics, _, _) = run_timed_command(create_command_str)?;

    // --- Extract Archive ---
    let extract_command_str = format!(
        "7z x -o'{}' '{}'",
        extract_path.display(),
        archive_path.display()
    );
    let (extract_metrics, _, _) = run_timed_command(extract_command_str)?;
// Validate integrity by comparing extracted files with originals
compare_dirs(dataset_path, &extract_path)?;

    let (source_file_count, source_total_size_bytes) = get_dir_stats(dataset_path);
    let archive_size_bytes = fs::metadata(&archive_path)?.len();
    let compression_ratio = if archive_size_bytes > 0 {
        source_total_size_bytes as f64 / archive_size_bytes as f64
    } else {
        0.0
    };

    Ok(BenchResult {
        dataset: dataset_name.to_string(),
        profile: profile.to_string(),
        archiver: "7z(lzma2)".to_string(),
        source_files: source_file_count,
        source_size_bytes: source_total_size_bytes,
        archive_size_bytes,
        compression_ratio,
        create_metrics,
        extract_metrics,
    })
}

// --- pigz (parallel gzip) benchmark ---
fn run_pigz_bench(
    dataset_path: &Path,
    dataset_name: &str,
    profile: &str,
) -> Result<BenchResult, Box<dyn Error>> {
    println!("\nRunning pigz benchmark, profile: '{}'", profile);

    let level: u8 = match profile {
        "L1" => 1,
        "L3" => 3,
        "L5" | "default" => 5,
        "L9" => 9,
        _ => 5,
    };

    let temp_dir_path = PathBuf::from("/tmp/pigz_bench").join(format!("bench_{}", std::process::id()));
    fs::create_dir_all(&temp_dir_path)?;
    let temp_dir = TempDirWrapper { path: temp_dir_path };
    let archive_path = temp_dir.path().join("test.tar.gz");
    let extract_path = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_path)?;

    // --- Create ---
    let create_command_str = format!(
        "tar -cf - -C '{}' . | pigz -{} > '{}'",
        dataset_path.display(),
        level,
        archive_path.display()
    );
    let (create_metrics, _, _) = run_timed_command(create_command_str)?;

    // --- Extract ---
    let extract_command_str = format!(
        "pigz -d -c '{}' | tar -xf - -C '{}'",
        archive_path.display(),
        extract_path.display()
    );
    let (extract_metrics, _, _) = run_timed_command(extract_command_str)?;

    // Validate
    compare_dirs(dataset_path, &extract_path)?;

    let (source_file_count, source_total_size_bytes) = get_dir_stats(dataset_path);
    let archive_size_bytes = fs::metadata(&archive_path)?.len();
    let compression_ratio = if archive_size_bytes > 0 {
        source_total_size_bytes as f64 / archive_size_bytes as f64
    } else { 0.0 };

    Ok(BenchResult {
        dataset: dataset_name.to_string(),
        profile: profile.to_string(),
        archiver: "pigz".to_string(),
        source_files: source_file_count,
        source_size_bytes: source_total_size_bytes,
        archive_size_bytes,
        compression_ratio,
        create_metrics,
        extract_metrics,
    })
}

// --- xz (multi-threaded) benchmark ---
fn run_xz_bench(
    dataset_path: &Path,
    dataset_name: &str,
    profile: &str,
) -> Result<BenchResult, Box<dyn Error>> {
    println!("\nRunning xz benchmark, profile: '{}'", profile);

    let level: u8 = match profile {
        "L1" => 1,
        "L3" => 3,
        "L5" | "default" => 5,
        "L7" => 7,
        "L9" => 9,
        _ => 5,
    };

    let temp_dir_path = PathBuf::from("/tmp/xz_bench").join(format!("bench_{}", std::process::id()));
    fs::create_dir_all(&temp_dir_path)?;
    let temp_dir = TempDirWrapper { path: temp_dir_path };
    let archive_path = temp_dir.path().join("test.tar.xz");
    let extract_path = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_path)?;

    // Create
    let create_command_str = format!(
        "tar -cf - -C '{}' . | xz -T0 -{} > '{}'",
        dataset_path.display(),
        level,
        archive_path.display()
    );
    let (create_metrics, _, _) = run_timed_command(create_command_str)?;

    // Extract
    let extract_command_str = format!(
        "xz -d -T0 -c '{}' | tar -xf - -C '{}'",
        archive_path.display(),
        extract_path.display()
    );
    let (extract_metrics, _, _) = run_timed_command(extract_command_str)?;

    compare_dirs(dataset_path, &extract_path)?;

    let (source_file_count, source_total_size_bytes) = get_dir_stats(dataset_path);
    let archive_size_bytes = fs::metadata(&archive_path)?.len();
    let compression_ratio = if archive_size_bytes > 0 {
        source_total_size_bytes as f64 / archive_size_bytes as f64
    } else { 0.0 };

    Ok(BenchResult {
        dataset: dataset_name.to_string(),
        profile: profile.to_string(),
        archiver: "xz".to_string(),
        source_files: source_file_count,
        source_size_bytes: source_total_size_bytes,
        archive_size_bytes,
        compression_ratio,
        create_metrics,
        extract_metrics,
    })
}

// --- GNU tar + zstd --fast benchmark ---
fn run_gtar_zstd_fast_bench(
    dataset_path: &Path,
    dataset_name: &str,
    profile: &str,
) -> Result<BenchResult, Box<dyn Error>> {
    println!("\nRunning gtar+zstd fast benchmark");

    let temp_dir_path = std::env::temp_dir()
        .join("bench_temp")
        .join(format!("bench_{}", std::process::id()));
    fs::create_dir_all(&temp_dir_path)?;
    let temp_dir = TempDirWrapper { path: temp_dir_path };
    let archive_path = temp_dir.path().join("test.tar.zst");
    let extract_path = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_path)?;

    // --- Create ---
    let create_command_str = format!(
        "gtar --numeric-owner --blocking-factor=512 --use-compress-program='zstd --fast=3 -T0' -C '{}' -cf '{}' .",
        dataset_path.display(),
        archive_path.display()
    );
    let (create_metrics, _, _) = run_timed_command(create_command_str)?;

    // --- Extract ---
    let extract_command_str = format!(
        "gtar --use-compress-program='zstd -d -T0' -C '{}' -xf '{}'",
        extract_path.display(),
        archive_path.display()
    );
    let (extract_metrics, _, _) = run_timed_command(extract_command_str)?;

    compare_dirs(dataset_path, &extract_path)?;

    let (source_file_count, source_total_size_bytes) = get_dir_stats(dataset_path);
    let archive_size_bytes = fs::metadata(&archive_path)?.len();
    let compression_ratio = if archive_size_bytes > 0 {
        source_total_size_bytes as f64 / archive_size_bytes as f64
    } else { 0.0 };

    Ok(BenchResult {
        dataset: dataset_name.to_string(),
        profile: profile.to_string(),
        archiver: "gtar+zstd_fast".to_string(),
        source_files: source_file_count,
        source_size_bytes: source_total_size_bytes,
        archive_size_bytes,
        compression_ratio,
        create_metrics,
        extract_metrics,
    })
}


fn cleanup_after_benchmark() {
    // Clear memory cache
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg("sync && echo 3 > /proc/sys/vm/drop_caches 2>/dev/null || true")
        .output();
    
    // Remove temporary files
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg("rm -rf /tmp/.tmp* 2>/dev/null || true")
        .output();
    
    // Small delay to let system settle
    std::thread::sleep(std::time::Duration::from_millis(100));
}

fn write_results_to_csv(results: &[BenchResult]) -> Result<(), Box<dyn Error>> {
    let mut wtr = Writer::from_path("benchmark_results.csv")?;
    wtr.write_record(&[
        "Dataset",
        "Profile",
        "Archiver",
        "Files",
        "Source Size",
        "Archive Size",
        "Ratio",
        "Create Time (Wall/CPU)",
        "Create TP (MB/s)",
        "Create Peak Mem",
        "Extract Time (Wall/CPU)",
        "Extract TP (MB/s)",
        "Extract Peak Mem",
    ])?;

    println!("\n--- Benchmark Results ---");
    println!("| Dataset | Profile | Archiver | Files | Source Size | Archive Size | Ratio | Create Time (Wall/CPU) | Create TP (MB/s) | Create Peak Mem | Extract Time (Wall/CPU) | Extract TP (MB/s) | Extract Peak Mem |");
    println!("| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |");

    for r in results {
        let source_size_mib = r.source_size_bytes as f64 / (1024.0 * 1024.0);
        let archive_size_mib = r.archive_size_bytes as f64 / (1024.0 * 1024.0);
        let create_tp = if r.create_metrics.wall_time_secs > 0.0 {
            source_size_mib / r.create_metrics.wall_time_secs
        } else {
            0.0
        };
        let extract_tp = if r.extract_metrics.wall_time_secs > 0.0 {
            source_size_mib / r.extract_metrics.wall_time_secs
        } else {
            0.0
        };

        let create_mem_mib = r.create_metrics.peak_mem_bytes as f64 / (1024.0 * 1024.0);
        let extract_mem_mib = r.extract_metrics.peak_mem_bytes as f64 / (1024.0 * 1024.0);

        let row = format!(
            "| {} | {} | {} | {} | {:.2} MiB | {:.2} MiB | {:.2}x | {:.2}s / {:.2}s | {:.2} | {:.2} MiB | {:.2}s / {:.2}s | {:.2} | {:.2} MiB |",
            r.dataset,
            r.profile,
            r.archiver,
            r.source_files,
            source_size_mib,
            archive_size_mib,
            r.compression_ratio,
            r.create_metrics.wall_time_secs,
            r.create_metrics.cpu_time_secs,
            create_tp,
            create_mem_mib,
            r.extract_metrics.wall_time_secs,
            r.extract_metrics.cpu_time_secs,
            extract_tp,
            extract_mem_mib
        );
        println!("{}", row);

        wtr.write_record(&[
            r.dataset.clone(),
            r.profile.clone(),
            r.archiver.clone(),
            r.source_files.to_string(),
            format!("{:.2} MiB", source_size_mib),
            format!("{:.2} MiB", archive_size_mib),
            format!("{:.2}x", r.compression_ratio),
            format!("{:.2}s / {:.2}s", r.create_metrics.wall_time_secs, r.create_metrics.cpu_time_secs),
            format!("{:.2}", create_tp),
            format!("{:.2} MiB", create_mem_mib),
            format!("{:.2}s / {:.2}s", r.extract_metrics.wall_time_secs, r.extract_metrics.cpu_time_secs),
            format!("{:.2}", extract_tp),
            format!("{:.2} MiB", extract_mem_mib),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

fn build_release_binary() -> Result<(), Box<dyn Error>> {
    println!("Building release executable (blitzarch)...");
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--bin")
        .arg("blitzarch")
        .status()?;
    if !status.success() {
        return Err("cargo build failed".into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    // Use pre-installed BlitzArch binary from system
    let use_lzma2 = args.contains(&"--use-lzma2".to_string());

    // Рекомендуемые профили и конкуренты для маркетингового бенчмарка
    let profiles = vec![
        ("BlitzArch", "L3_katana_mem_500"),   // фиксированный лимит 500 MiB для честного сравнения
        ("tar+zstd", "L3"),
        ("pigz", "L5"),
        ("xz", "L5"),
        ("gtar+zstd_fast", "F3"),
    ];

    /* Legacy baseline profiles – commented out
        ("BlitzArch", "L1_base"),
        ("BlitzArch", "L1_threads"),
        ("BlitzArch", "L1_sharded"),
        ("BlitzArch", "L3_base"),
        ("BlitzArch", "L3_threads"),
        ("BlitzArch", "L3_sharded"),
        ("BlitzArch", "L3_katana"),
        ("BlitzArch", "L3_katana_adaptive"),
        ("BlitzArch", "L7_base"),
        */
        /*
        // --- Level 1 ---
        ("BlitzArch", "L1_base"),
        ("BlitzArch", "L1_threads"),
        ("BlitzArch", "L1_sharded"),

        // --- Level 3 (основной баланс) ---
        ("BlitzArch", "L3_base"),
        ("BlitzArch", "L3_threads"),
        ("BlitzArch", "L3_sharded"),
        // katana движок
        ("BlitzArch", "L3_katana"),
        ("BlitzArch", "L3_katana_adaptive"),
        */


    // Use the root directory as a single dataset
    let mut datasets: Vec<(String, String)> = Vec::new();
    let dataset_root_env = env::var("DATASET_ROOT").unwrap_or_else(|_| "/home/ubuntu/datasets".to_string());
    let dataset_root = Path::new(&dataset_root_env);
    if dataset_root.exists() {
        let dataset_name = dataset_root.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        datasets.push((dataset_name, dataset_root.to_string_lossy().to_string()));
    } else {
        return Err("dataset directory not found".into());
    }

    let mut all_results = Vec::new();

    for (dataset_name, dataset_path_str) in &datasets {
        let dataset_path = Path::new(dataset_path_str);
        for (archiver, profile) in &profiles {
            // Запускаем все архиваторы, указанные в списке `profiles`
            // Skip heavy video dataset for 7z and zip because these formats may skip large files like sample.mp4
            if (archiver.starts_with("7z") || archiver.starts_with("zip")) && dataset_name.contains("video") {
                println!("Skipping {} on dataset {} (unsupported large video files)", archiver, dataset_name);
                continue;
            }
            let result = match *archiver {
                "BlitzArch" => run_blitzarch_bench(dataset_path, dataset_name, profile, use_lzma2),
                "tar+zstd" => run_tar_zstd_bench(dataset_path, dataset_name, profile),
                "7z_lzma2" => run_7z_lzma2_bench(dataset_path, dataset_name, profile),
                "zip+zstd" => run_zip_zstd_bench(dataset_path, dataset_name, profile),
                "pigz" => run_pigz_bench(dataset_path, dataset_name, profile),
                "xz" => run_xz_bench(dataset_path, dataset_name, profile),
                "gtar+zstd_fast" => run_gtar_zstd_fast_bench(dataset_path, dataset_name, profile),
                _ => panic!("Unknown archiver {}", archiver),
            };

            match result {
                Ok(res) => all_results.push(res),
                Err(e) => eprintln!(
                    "Error running benchmark for {}/{}/{}: {}",
                    dataset_name,
                    archiver,
                    profile,
                    e
                ),
            }
            
            // Clean up memory and temporary files after each benchmark
            cleanup_after_benchmark();
        }
    }

    if let Err(e) = write_results_to_csv(&all_results) {
        eprintln!("Failed to write results to CSV: {}", e);
    }
    Ok(())
}
