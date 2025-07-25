use std::path::Path;
use std::time::{SystemTime, Duration};
use std::process::Command;
use serde::{Deserialize, Serialize};

/// Converts `Some(String)` into `None` if the string is empty or contains only
/// whitespace. This prevents accidentally treating an empty password field as
/// a real password which would turn on encryption for new archives or require
/// a password during extraction.
fn normalize_password(p: Option<String>) -> Option<String> {
    match p {
        Some(s) if s.trim().is_empty() => None,
        other => other,
    }
}
use std::fs;
use sysinfo::{System, Disks};
use tauri::AppHandle;
use tauri::Emitter;


// Import BlitzArch engine functions and types
use blitzarch::katana::{create_katana_archive_with_progress, extract_katana_archive_with_progress};
use blitzarch::progress::ProgressState;

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveStats {
    pub files: Option<u64>,
    pub time_sec: Option<f64>,
    pub ratio: Option<f64>,
    pub speed_mb_s: Option<f64>,
    pub total_bytes: Option<u64>,
    pub archive_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub archive_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<ArchiveStats>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub memory_percentage: f32,
    pub disk_usage: f32,
    pub disk_read_bytes: u64,
    pub disk_written_bytes: u64,
}

/// Возвращает уникальный путь, добавляя суффикс " copy" аналогично macOS Finder.
/// Пример: photo.png -> photo copy.png, photo copy 2.png, ...
fn generate_unique_path(original: &Path) -> std::path::PathBuf {
    if !original.exists() {
        return original.to_path_buf();
    }

    let stem = original.file_stem().unwrap_or_default().to_string_lossy();
    let ext = original.extension().map(|e| e.to_string_lossy());

    for idx in 1.. {
        let candidate = if idx == 1 {
            if let Some(ext) = &ext {
                original.with_file_name(format!("{} copy.{}", stem, ext))
            } else {
                original.with_file_name(format!("{} copy", stem))
            }
        } else {
            if let Some(ext) = &ext {
                original.with_file_name(format!("{} copy {}.{}", stem, idx, ext))
            } else {
                original.with_file_name(format!("{} copy {}", stem, idx))
            }
        };

        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("generate_unique_path loop should always return");
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProgressEvent {
    pub operation: String,           // "create" or "extract"
    pub progress: f32,              // 0.0 to 100.0
    pub speed: f32,                // MB/s
    pub message: String,           // Status message
    pub completed: bool,           // True when operation is finished
    pub error: Option<String>,     // Error message if any
    
    // Rich metrics from ProgressState
    pub processed_files: u64,      // Number of files processed
    pub total_files: u64,          // Total number of files
    pub processed_bytes: u64,      // Bytes processed
    pub total_bytes: u64,          // Total bytes
    pub completed_shards: u32,     // Completed shards
    pub total_shards: u32,         // Total shards
    pub elapsed_time: f32,         // Elapsed time in seconds
    pub eta_seconds: f32,          // Estimated time remaining
    pub compression_ratio: Option<f32>, // Compression ratio (for completed operations)
}

// Async version of create_archive with progress events (RECREATED)
#[tauri::command(async)]
pub async fn create_archive_async(
    app: AppHandle,
    inputs: Vec<String>,
    output_path: String,
    compression_level: u32,
    bundle_size: Option<u64>,
    password: Option<String>,
    threads: Option<usize>,
    codec_threads: Option<u32>,
    memory_budget: Option<u64>,
) -> Result<ArchiveResult, String> {
    let app_clone = app.clone();
    let inputs_clone = inputs.clone();
    let output_path_clone = output_path.clone();
    
    // Use engine directly instead of CLI spawning for real progress
    tauri::async_runtime::spawn_blocking(move || {
        create_archive_with_real_progress(
            app_clone,
            inputs_clone,
            output_path_clone,
            compression_level,
            bundle_size,
            password,
            threads,
            codec_threads,
            memory_budget,
        )
    }).await.map_err(|e| format!("Task execution failed: {}", e))?
}

fn create_archive_with_real_progress(
    app: AppHandle,
    inputs: Vec<String>,
    output_path: String,
    _compression_level: u32,
    _bundle_size: Option<u64>,
    password: Option<String>,
    threads: Option<usize>,
    codec_threads: Option<u32>,
    memory_budget: Option<u64>,
) -> Result<ArchiveResult, String> {
    println!("🚀 Creating archive async: {}", output_path);
    
    let password = normalize_password(password);
    
    let start_time = std::time::Instant::now();
    
    // Initial progress
    let initial_progress = ProgressEvent {
        operation: "create".to_string(),
        progress: 0.0,
        speed: 0.0,
        message: "Starting archive creation...".to_string(),
        completed: false,
        error: None,
        
        // Initialize metrics
        processed_files: 0,
        total_files: 0,
        processed_bytes: 0,
        total_bytes: 0,
        completed_shards: 0,
        total_shards: 0,
        elapsed_time: 0.0,
        eta_seconds: 0.0,
        compression_ratio: None,
    };
    app.emit("archive-progress", &initial_progress).ok();
    
    // Convert string paths to PathBuf
    let input_paths: Vec<std::path::PathBuf> = inputs.iter().map(|s| std::path::PathBuf::from(s)).collect();
    let output_pathbuf = std::path::PathBuf::from(&output_path);
    
    // Store last progress state for final stats
    let last_progress_state = std::sync::Arc::new(std::sync::Mutex::new(None::<ProgressState>));
    let last_progress_clone = last_progress_state.clone();
    
    // Create progress callback
    let app_for_progress = app.clone();
    let progress_callback = move |state: ProgressState| {
        let progress_event = ProgressEvent {
            operation: "create".to_string(),
            progress: state.progress_percent,
            speed: state.speed_mbps,
            message: format!("Processing files: {}/{} ({:.1} MB/s)", 
                state.processed_files, 
                state.total_files, 
                state.speed_mbps),
            completed: false,
            error: None,
            
            // Rich metrics from ProgressState
            processed_files: state.processed_files,
            total_files: state.total_files,
            processed_bytes: state.processed_bytes,
            total_bytes: state.total_bytes,
            completed_shards: state.completed_shards,
            total_shards: state.total_shards,
            elapsed_time: state.elapsed_time.as_secs_f32(),
            eta_seconds: state.estimated_time_remaining().as_secs_f32(),
            compression_ratio: None, // Will be set in final event
        };
        app_for_progress.emit("archive-progress", &progress_event).ok();
    };
    
    // Call engine directly with progress callback
    // Обеспечиваем уникальный путь архива при асинхронном варианте
let output_pathbuf = generate_unique_path(Path::new(&output_path));
let output_path = output_pathbuf.to_string_lossy().to_string();

let result = create_katana_archive_with_progress(
        &input_paths,
        &output_pathbuf,
        threads.unwrap_or(0),
        codec_threads.unwrap_or(0),
        memory_budget,
        password,
        Some(progress_callback),
    );
    
    // Handle result and emit final progress
    match result {
        Ok(()) => {
            let elapsed = start_time.elapsed();
            
            // Calculate final metrics with recursive directory traversal
            let archive_size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
            
            // Recursively calculate total size and file count
            let (total_input_size, actual_file_count) = calculate_recursive_stats(&input_paths);
            
            fn calculate_recursive_stats(paths: &[std::path::PathBuf]) -> (u64, u64) {
                let mut total_size = 0u64;
                let mut file_count = 0u64;
                
                for path in paths {
                    if let Ok(metadata) = std::fs::metadata(path) {
                        if metadata.is_file() {
                            total_size += metadata.len();
                            file_count += 1;
                        } else if metadata.is_dir() {
                            if let Some(path_str) = path.to_str() {
                                let (dir_size, dir_files) = calculate_dir_stats(path_str);
                                total_size += dir_size;
                                file_count += dir_files;
                            }
                        }
                    }
                }
                
                (total_size, file_count)
            }
            
            fn calculate_dir_stats(dir_path: &str) -> (u64, u64) {
                let mut total_size = 0u64;
                let mut file_count = 0u64;
                
                if let Ok(entries) = std::fs::read_dir(dir_path) {
                    for entry in entries.flatten() {
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.is_file() {
                                total_size += metadata.len();
                                file_count += 1;
                            } else if metadata.is_dir() {
                                if let Some(path_str) = entry.path().to_str() {
                                    let (sub_size, sub_files) = calculate_dir_stats(path_str);
                                    total_size += sub_size;
                                    file_count += sub_files;
                                }
                            }
                        }
                    }
                }
                
                (total_size, file_count)
            }
            
            let compression_ratio = if archive_size > 0 {
                Some(total_input_size as f32 / archive_size as f32)
            } else {
                None
            };
            
            let final_progress = ProgressEvent {
                operation: "create".to_string(),
                progress: 100.0,
                speed: if elapsed.as_secs_f32() > 0.0 {
                    (total_input_size as f32 / (1024.0 * 1024.0)) / elapsed.as_secs_f32()
                } else { 0.0 },
                message: format!("Archive created successfully in {:.1}s", elapsed.as_secs_f32()),
                completed: true,
                error: None,
                
                // Final metrics from last progress state if available
                processed_files: if let Ok(last_state) = last_progress_state.lock() {
                    last_state.as_ref().map(|s| s.processed_files).unwrap_or(actual_file_count)
                } else { actual_file_count },
                total_files: if let Ok(last_state) = last_progress_state.lock() {
                    last_state.as_ref().map(|s| s.total_files).unwrap_or(actual_file_count)
                } else { actual_file_count },
                processed_bytes: total_input_size,
                total_bytes: total_input_size,
                completed_shards: 1, // Approximation
                total_shards: 1,
                elapsed_time: elapsed.as_secs_f32(),
                eta_seconds: 0.0,
                compression_ratio,
            };
            app.emit("archive-progress", &final_progress).ok();
            
            // Use the actual file count calculated recursively
            let final_stats = ArchiveStats {
                files: Some(actual_file_count),
                time_sec: Some(elapsed.as_secs_f64()),
                ratio: compression_ratio.map(|r| r as f64),
                speed_mb_s: if elapsed.as_secs_f64() > 0.0 {
                    Some((total_input_size as f64 / 1_048_576.0) / elapsed.as_secs_f64())
                } else { None },
                total_bytes: Some(total_input_size),
                archive_bytes: Some(archive_size),
            };

            Ok(ArchiveResult {
                success: true,
                output: Some(format!("Archive created successfully: {}", output_path)),
                error: None,
                archive_path: Some(output_path.clone()),
                stats: Some(final_stats),
            })
        }
        Err(e) => {
            let error_msg = format!("Archive creation failed: {}", e);
            let final_progress = ProgressEvent {
                operation: "create".to_string(),
                progress: 0.0,
                speed: 0.0,
                message: "Archive creation failed!".to_string(),
                completed: true,
                error: Some(error_msg.clone()),
                
                // Error state metrics
                processed_files: 0,
                total_files: 0,
                processed_bytes: 0,
                total_bytes: 0,
                completed_shards: 0,
                total_shards: 0,
                elapsed_time: start_time.elapsed().as_secs_f32(),
                eta_seconds: 0.0,
                compression_ratio: None,
            };
            app.emit("archive-progress", &final_progress).ok();
            
            Ok(ArchiveResult {
                success: false,
                output: None,
                error: Some(error_msg),
                archive_path: None,
                stats: None,
            })
        }
    }
}

// Tauri command to create archive (legacy sync version)
#[tauri::command]
pub fn create_archive(
    files: Vec<String>,
    archive_name: String,
    output_dir: String,
    compression_level: Option<u8>,
    password: Option<String>,
    bundle_size: Option<u32>,
) -> Result<ArchiveResult, String> {
    println!("🚀 Creating archive: {} in {}", archive_name, output_dir);
    println!("📋 Files: {:?}", files);
    
    let password = normalize_password(password);
    
    // Build BlitzArch command
    let mut cmd = Command::new("/Users/oleksandr/Desktop/Development/blitzarch/target/release/blitzarch");
    cmd.arg("create");
    
    // Output path
    let mut archive_pathbuf = Path::new(&output_dir).join(format!("{}.blz", archive_name));
// Автоматический выбор уникального имени архива
archive_pathbuf = generate_unique_path(&archive_pathbuf);
let archive_path = archive_pathbuf.to_string_lossy().to_string();
    cmd.args(["--output", &archive_path]);
    
    // Bundle size
    let bundle_size = bundle_size.unwrap_or(32);
    cmd.args(["--bundle-size", &bundle_size.to_string()]);
    
    // Compression level
    if let Some(level) = compression_level {
        if level != 3 {
            cmd.args(["--level", &level.to_string()]);
        }
    }
    
    // Password
    if let Some(pwd) = password {
        cmd.args(["--password", &pwd]);
    }
    
    // Add input files
    for file in &files {
        cmd.arg(file);
    }
    
    println!("🔧 Command: {:?}", cmd);
    
    // Execute command
    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            if output.status.success() {
                println!("✅ Archive created successfully");
                Ok(ArchiveResult {
                    success: true,
                    output: Some(stdout.to_string()),
                    error: None,
                    archive_path: Some(archive_path),
                    stats: None,
                })
            } else {
                println!("❌ Archive creation failed: {}", stderr);
                Ok(ArchiveResult {
                    success: false,
                    output: None,
                    error: Some(stderr.to_string()),
                    archive_path: None,
                    stats: None,
                })
            }
        }
        Err(e) => {
            println!("💥 Command execution failed: {}", e);
            Err(format!("Failed to execute command: {}", e))
        }
    }
}

// Tauri command to get downloads directory path
#[tauri::command]
pub fn get_downloads_path() -> Result<String, String> {
    // Get user's home directory and append Downloads
    let home_dir = dirs::home_dir()
        .ok_or("Failed to get home directory")?;
    let downloads_path = home_dir.join("Downloads");
    
    // Create Downloads directory if it doesn't exist
    if !downloads_path.exists() {
        fs::create_dir_all(&downloads_path)
            .map_err(|e| format!("Failed to create Downloads directory: {}", e))?;
    }
    
    Ok(downloads_path.to_string_lossy().to_string())
}

// Tauri command to get parent directory of a file
#[tauri::command]
pub fn get_parent_directory(file_path: String) -> Result<String, String> {
    let path = Path::new(&file_path);
    if let Some(parent) = path.parent() {
        Ok(parent.to_string_lossy().to_string())
    } else {
        Err("Could not get parent directory".to_string())
    }
}

// Async version of extract_archive with progress events
#[tauri::command(async)]
pub async fn extract_archive_async(
    app: AppHandle,
    archive_path: String,
    output_dir: String,
    password: Option<String>,
    strip_components: Option<u32>,
    specific_files: Option<Vec<String>>,
) -> Result<ArchiveResult, String> {
    let app_clone = app.clone();
    let archive_path_clone = archive_path.clone();
    let output_dir_clone = output_dir.clone();
    
    // Use engine directly instead of CLI spawning for real progress
    tauri::async_runtime::spawn_blocking(move || {
        extract_archive_with_real_progress(
            app_clone,
            archive_path_clone,
            output_dir_clone,
            password,
            strip_components,
            specific_files,
        )
    }).await.map_err(|e| format!("Task execution failed: {}", e))?
}

fn extract_archive_with_real_progress(
    app: AppHandle,
    archive_path: String,
    output_dir: String,
    password: Option<String>,
    strip_components: Option<u32>,
    specific_files: Option<Vec<String>>,
) -> Result<ArchiveResult, String> {
    println!("🔄 Extracting archive async: {} to {}", archive_path, output_dir);
    println!("🔧 Debug params: strip_components={:?}, specific_files={:?}", strip_components, specific_files);
    
    let password = normalize_password(password);
    
    let start_time = std::time::Instant::now();
    
    // Initial progress
    let initial_progress = ProgressEvent {
        operation: "extract".to_string(),
        progress: 0.0,
        speed: 0.0,
        message: "Starting archive extraction...".to_string(),
        completed: false,
        error: None,
        
        // Initialize rich metrics
        processed_files: 0,
        total_files: 0,
        processed_bytes: 0,
        total_bytes: 0,
        completed_shards: 0,
        total_shards: 0,
        elapsed_time: 0.0,
        eta_seconds: 0.0,
        compression_ratio: None,
    };
    app.emit("archive-progress", &initial_progress).ok();
    
    // Convert string paths to PathBuf
    let archive_pathbuf = std::path::PathBuf::from(&archive_path);
    let output_pathbuf = std::path::PathBuf::from(&output_dir);
    
    // Expand directories in specific_files into individual file paths
    let specs_vec: Vec<String> = specific_files.clone().unwrap_or_default();

    let expanded_files: Vec<String> = if specs_vec.is_empty() {
        Vec::new()
    } else {
        match read_archive_index(&archive_path, password.clone()) {
            Ok(entries) => {
                let mut files = Vec::new();
                for spec in &specs_vec {
                    // Treat the spec as a prefix for directory matching (with or without trailing slash)
                    let prefix = format!("{}/", spec.trim_end_matches('/'));
                    for entry in &entries {
                        if entry.is_dir {
                            continue;
                        }
                        if entry.path == *spec || entry.path.starts_with(&prefix) {
                            files.push(entry.path.clone());
                        }
                    }
                }
                if files.is_empty() {
                    specs_vec.clone()
                } else {
                    files
                }
            }
            Err(_) => specs_vec.clone(),
        }
    };

    // Store empty status before move
    let is_full_extraction = expanded_files.is_empty();
    
    let selected: Vec<std::path::PathBuf> = expanded_files
        .clone()
        .into_iter()
        .map(|s| std::path::PathBuf::from(s))
        .collect();

    // Create progress callback
    let app_for_progress = app.clone();
    let progress_callback = move |state: ProgressState| {
        let progress_event = ProgressEvent {
            operation: "extract".to_string(),
            progress: state.progress_percent,
            speed: state.speed_mbps,
            message: format!("Extracting files: {}/{} ({:.1} MB/s)", 
                state.processed_files, 
                state.total_files, 
                state.speed_mbps),
            completed: false,
            error: None,
            
            // Rich metrics from ProgressState
            processed_files: state.processed_files,
            total_files: state.total_files,
            processed_bytes: state.processed_bytes,
            total_bytes: state.total_bytes,
            completed_shards: state.completed_shards,
            total_shards: state.total_shards,
            elapsed_time: state.elapsed_time.as_secs_f32(),
            eta_seconds: state.estimated_time_remaining().as_secs_f32(),
            compression_ratio: None, // Will be set in final event
        };
        app_for_progress.emit("archive-progress", &progress_event).ok();
    };
    
    // Ensure output directory exists
    if let Err(e) = std::fs::create_dir_all(&output_pathbuf) {
        println!("❌ Failed to create output directory: {}", e);
        return Err(format!("Failed to create output directory: {}", e));
    }
    println!("✅ Output directory verified: {}", output_dir);
    
    // Call engine directly with progress
    println!("🚀 Calling extract_katana_archive_with_progress...");
    let result = extract_katana_archive_with_progress(
        &archive_pathbuf,
        &output_pathbuf,
        &selected, // empty = all files
        password.clone(),
        strip_components,
        Some(progress_callback),
    );
    
    // Handle result and emit final progress
    match result {
        Ok(()) => {
            let elapsed = start_time.elapsed();
            
            // Calculate extracted files count and size
            let (extracted_bytes, extracted_files_count) = if is_full_extraction {
                // Full extraction - calculate from archive contents
                match read_archive_index(&archive_path, password.clone()) {
                    Ok(entries) => {
                        let files: Vec<_> = entries.iter().filter(|e| !e.is_dir).collect();
                        let total_size: u64 = files.iter().map(|e| e.size).sum();
                        (total_size, files.len() as u64)
                    }
                    Err(_) => (0, 1) // Fallback
                }
            } else {
                // Partial extraction - calculate from selected files
                match read_archive_index(&archive_path, password.clone()) {
                    Ok(entries) => {
                        let mut total_size = 0u64;
                        let mut file_count = 0u64;
                        for spec in &expanded_files {
                            if let Some(entry) = entries.iter().find(|e| e.path == *spec && !e.is_dir) {
                                total_size += entry.size;
                                file_count += 1;
                            }
                        }
                        (total_size, file_count)
                    }
                    Err(_) => (0, expanded_files.len() as u64) // Fallback
                }
            };
            
            let final_progress = ProgressEvent {
                operation: "extract".to_string(),
                progress: 100.0,
                speed: if elapsed.as_secs_f32() > 0.0 {
                    (extracted_bytes as f32 / (1024.0 * 1024.0)) / elapsed.as_secs_f32()
                } else { 0.0 },
                message: format!("Extraction completed successfully in {:.1}s", elapsed.as_secs_f32()),
                completed: true,
                error: None,
                
                // Real final metrics
                processed_files: extracted_files_count,
                total_files: extracted_files_count,
                processed_bytes: extracted_bytes,
                total_bytes: extracted_bytes,
                completed_shards: 1,
                total_shards: 1,
                elapsed_time: elapsed.as_secs_f32(),
                eta_seconds: 0.0,
                compression_ratio: None,
            };
            app.emit("archive-progress", &final_progress).ok();
            
            // Create proper extraction stats
            let final_stats = ArchiveStats {
                files: Some(extracted_files_count),
                time_sec: Some(elapsed.as_secs_f64()),
                ratio: None, // N/A for extraction
                speed_mb_s: if elapsed.as_secs_f64() > 0.0 {
                    Some((extracted_bytes as f64 / 1_048_576.0) / elapsed.as_secs_f64())
                } else { None },
                total_bytes: Some(extracted_bytes),
                archive_bytes: None, // N/A for extraction
            };
            
            Ok(ArchiveResult {
                success: true,
                output: Some(format!("Archive extracted successfully: {}", output_dir)),
                error: None,
                archive_path: Some(archive_path),
                stats: Some(final_stats),
            })
        }
        Err(e) => {
            let error_msg = format!("Archive extraction failed: {}", e);
            let final_progress = ProgressEvent {
                operation: "extract".to_string(),
                progress: 0.0,
                speed: 0.0,
                message: "Archive extraction failed!".to_string(),
                completed: true,
                error: Some(error_msg.clone()),
                
                // Error state metrics
                processed_files: 0,
                total_files: 0,
                processed_bytes: 0,
                total_bytes: 0,
                completed_shards: 0,
                total_shards: 0,
                elapsed_time: start_time.elapsed().as_secs_f32(),
                eta_seconds: 0.0,
                compression_ratio: None,
            };
            app.emit("archive-progress", &final_progress).ok();
            
            Ok(ArchiveResult {
                success: false,
                output: None,
                error: Some(error_msg),
                archive_path: None,
                stats: None,
            })
        }
    }
}

// Tauri command to extract archive (legacy sync version)
#[tauri::command]
pub fn extract_archive(
    archive_path: String,
    output_dir: String,
    password: Option<String>,
    strip_components: Option<u32>,
) -> Result<ArchiveResult, String> {
    println!("🔄 Extracting archive: {} to {}", archive_path, output_dir);
    
    let password = normalize_password(password);
    
    // Build BlitzArch extract command
    let mut cmd = Command::new("/Users/oleksandr/Desktop/Development/blitzarch/target/release/blitzarch");
    cmd.arg("extract");
    
    // Input archive
    cmd.arg(&archive_path);
    
    // Output directory
    cmd.args(["--output", &output_dir]);
    
    // Strip components if specified
    if let Some(n) = strip_components {
        cmd.args(["--strip-components", &n.to_string()]);
    }
    
    // Password if provided
    if let Some(pwd) = password {
        cmd.args(["--password", &pwd]);
    }
    
    println!("🔧 Extract command: {:?}", cmd);
    
    // Execute command
    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            if output.status.success() {
                println!("✅ Archive extracted successfully");
                Ok(ArchiveResult {
                    success: true,
                    output: Some(stdout.to_string()),
                    error: None,
                    archive_path: None,
                stats: None,
                })
            } else {
                println!("❌ Archive extraction failed: {}", stderr);
                Ok(ArchiveResult {
                    success: false,
                    output: None,
                    error: Some(stderr.to_string()),
                    archive_path: None,
                stats: None,
                })
            }
        }
        Err(e) => {
            println!("💥 Extract command execution failed: {}", e);
            Err(format!("Failed to execute extract command: {}", e))
        }
    }
}

// Tauri command to list archive contents
#[tauri::command]
pub fn list_archive(archive_path: String) -> Result<ArchiveResult, String> {
    println!("📋 Listing archive contents: {}", archive_path);
    
    // Build BlitzArch list command
    let mut cmd = Command::new("/Users/oleksandr/Desktop/Development/blitzarch/target/release/blitzarch");
    cmd.arg("list");
    cmd.arg(&archive_path);
    
    println!("🔧 List command: {:?}", cmd);
    
    // Execute command
    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            if output.status.success() {
                println!("✅ Archive listed successfully");
                Ok(ArchiveResult {
                    success: true,
                    output: Some(stdout.to_string()),
                    error: None,
                    archive_path: None,
                stats: None,
                })
            } else {
                println!("❌ Archive listing failed: {}", stderr);
                Ok(ArchiveResult {
                    success: false,
                    output: None,
                    error: Some(stderr.to_string()),
                    archive_path: None,
                stats: None,
                })
            }
        }
        Err(e) => {
            println!("💥 List command execution failed: {}", e);
            Err(format!("Failed to execute list command: {}", e))
        }
    }
}

// Tauri command to extract single file for drag-out
#[tauri::command]
pub async fn drag_out_extract(
    app: AppHandle,
    archive_path: String,
    file_path: String,
    target_dir: String,
    mut password: Option<String>,
) -> Result<ArchiveResult, String> {
    println!("🎯 Drag-out extracting file: {} from {}", file_path, archive_path);
    
        // Normalize empty password value coming from the frontend
    password = normalize_password(password);
    // Create target directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&target_dir) {
        return Ok(ArchiveResult {
            success: false,
            output: None,
            error: Some(format!("Failed to create target directory: {}", e)),
            archive_path: None,
                stats: None,
        });
    }
    
    // Extract single file to target directory
    let specific_files = vec![file_path.clone()];
    
    // Use existing extract logic
    let result = extract_archive_async(
        app,
        archive_path.clone(),
        target_dir.clone(),
        password,
        Some(0), // No strip_components for drag-out
        Some(specific_files),
    ).await;
    
    match result {
        Ok(mut archive_result) => {
            // Set the actual file path for the extracted file (just filename for drag-out)
            let file_name = std::path::Path::new(&file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("extracted_file");
            // Уникализируем имя, если файл уже существует в target_dir
let dest_candidate = Path::new(&target_dir).join(file_name);
let unique_dest = generate_unique_path(&dest_candidate);
let extracted_file_path = unique_dest.to_string_lossy().to_string();
            archive_result.archive_path = Some(extracted_file_path.clone());
            println!("✅ Drag-out extraction successful: {:?}", archive_result.archive_path);
            Ok(archive_result)
        }
        Err(e) => {
            println!("❌ Drag-out extraction failed: {}", e);
            Ok(ArchiveResult {
                success: false,
                output: None,
                error: Some(e),
                archive_path: None,
                stats: None,
            })
        }
    }
}

// Tauri command to create link file (.webloc or .url)
#[tauri::command]
pub fn create_link_file(path: String, contents: String) -> Result<ArchiveResult, String> {
    match fs::write(&path, contents) {
        Ok(_) => Ok(ArchiveResult { success: true, output: Some(path.clone()), error: None, archive_path: Some(path), stats: None }),
        Err(e) => Ok(ArchiveResult { success: false, output: None, error: Some(e.to_string()), archive_path: None, stats: None }),
    }
}

// Tauri command to cleanup drag-out temporary directory
#[tauri::command]
pub fn cleanup_drag_out_temp(temp_dir: String, max_age_hours: Option<u64>) -> Result<ArchiveResult, String> {
    println!("🧹 Cleaning up drag-out temp directory: {}", temp_dir);

    // Ensure directory exists; if not, nothing to clean
    if !std::path::Path::new(&temp_dir).exists() {
        return Ok(ArchiveResult {
            success: true,
            output: Some("Temp directory does not exist. Nothing to clean.".to_string()),
            error: None,
            archive_path: None,
                stats: None,
        });
    }

    let max_age_secs = max_age_hours.unwrap_or(24) * 3600;
    let now = SystemTime::now();
    let mut removed: u64 = 0;
    let mut errors: Vec<String> = Vec::new();

    match fs::read_dir(&temp_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                match entry.metadata() {
                    Ok(meta) => {
                        if let Ok(modified) = meta.modified() {
                            if now.duration_since(modified).unwrap_or(Duration::from_secs(0)) > Duration::from_secs(max_age_secs) {
                                let res = if meta.is_dir() {
                                    fs::remove_dir_all(&path)
                                } else {
                                    fs::remove_file(&path)
                                };
                                match res {
                                    Ok(_) => {
                                        removed += 1;
                                    }
                                    Err(e) => {
                                        errors.push(format!("{}: {}", path.display(), e));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => errors.push(format!("{}: {}", path.display(), e)),
                }
            }
        }
        Err(e) => {
            return Ok(ArchiveResult {
                success: false,
                output: None,
                error: Some(format!("Failed to read dir: {}", e)),
                archive_path: None,
                stats: None,
            });
        }
    }

    if errors.is_empty() {
        Ok(ArchiveResult {
            success: true,
            output: Some(format!("Removed {} old items", removed)),
            error: None,
            archive_path: None,
                stats: None,
        })
    } else {
        Ok(ArchiveResult {
            success: false,
            output: Some(format!("Removed {} old items", removed)),
            error: Some(errors.join("; ")),
            archive_path: None,
                stats: None,
        })
    }
}

// Tauri command to delete file
#[tauri::command]
pub fn delete_file(file_path: String) -> Result<ArchiveResult, String> {
    println!("🗑️ Deleting file: {}", file_path);
    
    match fs::remove_file(&file_path) {
        Ok(_) => {
            println!("✅ File deleted successfully");
            Ok(ArchiveResult {
                success: true,
                output: Some(format!("File deleted: {}", file_path)),
                error: None,
                archive_path: None,
                stats: None,
            })
        }
        Err(e) => {
            println!("❌ File deletion failed: {}", e);
            Ok(ArchiveResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                archive_path: None,
                stats: None,
            })
        }
    }
}

// Tauri command to get system metrics
#[tauri::command]
pub fn get_system_metrics() -> Result<SystemMetrics, String> {
    let mut sys = System::new();
    sys.refresh_all();
    
    // Get CPU usage (average across all cores)
    let cpu_usage = sys.global_cpu_info().cpu_usage();
    
    // Get memory usage
    let memory_used = sys.used_memory();
    let memory_total = sys.total_memory();
    let memory_percentage = if memory_total > 0 {
        (memory_used as f32 / memory_total as f32) * 100.0
    } else {
        0.0
    };
    
    // Get disk I/O stats (create separate disks instance)
    let mut total_disk_usage = 0.0;
    let total_read_bytes = 0;
    let total_written_bytes = 0;
    let disks = Disks::new_with_refreshed_list();
    
    if !disks.is_empty() {
        for disk in &disks {
            // For disk usage percentage, we'll use available space
            let total_space = disk.total_space();
            let available_space = disk.available_space();
            if total_space > 0 {
                let used_space = total_space - available_space;
                let usage_percent = (used_space as f64 / total_space as f64) * 100.0;
                total_disk_usage += usage_percent as f32;
            }
        }
        total_disk_usage /= disks.len() as f32; // Average disk usage
    }
    
    Ok(SystemMetrics {
        cpu_usage,
        memory_used,
        memory_total,
        memory_percentage,
        disk_usage: total_disk_usage,
        disk_read_bytes: total_read_bytes,
        disk_written_bytes: total_written_bytes,
    })
}

// Native drag-out global command (macOS only wrapper)


// Async command to list archive contents without blocking the GUI
#[tauri::command(async)]
pub async fn list_archive_async(
    archive_path: String,
    password: Option<String>,
) -> Result<Vec<ArchiveEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        match read_archive_index(&archive_path, password) {
            Ok(list) => Ok(list),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Internal helper that returns archive entries by reading Katana index
fn read_archive_index(archive_path: &str, _password: Option<String>) -> Result<Vec<ArchiveEntry>, Box<dyn std::error::Error>> {
    use std::io::{Read, Seek, SeekFrom};
    use zstd::decode_all;
    use serde::Deserialize;


    let mut f = std::fs::File::open(archive_path)?;
    let len = f.metadata()?.len();
    if len < 24 {
        return Err("File too small or not a Katana archive".into());
    }

    // Read footer (index sizes + magic)
    f.seek(SeekFrom::End(-24))?;
    let mut buf_footer = [0u8; 24];
    f.read_exact(&mut buf_footer)?;
    let (idx_comp_size_bytes, rest) = buf_footer.split_at(8);
    let (idx_json_size_bytes, magic_bytes) = rest.split_at(8);
    const LOCAL_KATANA_MAGIC: &[u8; 8] = b"KATIDX01";
    if magic_bytes != LOCAL_KATANA_MAGIC {
        return Err("Not a Katana archive".into());
    }
    let idx_comp_size = u64::from_le_bytes(idx_comp_size_bytes.try_into().unwrap());
    let _idx_json_size = u64::from_le_bytes(idx_json_size_bytes.try_into().unwrap());

    // Read compressed index
    let idx_comp_offset = len - 24 - idx_comp_size;
    f.seek(SeekFrom::Start(idx_comp_offset))?;
    let mut idx_comp = vec![0u8; idx_comp_size as usize];
    f.read_exact(&mut idx_comp)?;
    let idx_json = decode_all(&*idx_comp)?;
    #[derive(Deserialize)]
    struct IndexFile { path: String, size: u64 }
    #[derive(Deserialize)]
    struct RootIndex { files: Vec<IndexFile> }
    let index: RootIndex = serde_json::from_slice(&idx_json)?;

    // Map FileEntry -> ArchiveEntry
    let mut entries = Vec::with_capacity(index.files.len());
    for file in index.files {
        entries.push(ArchiveEntry {
            path: file.path,
            size: file.size,
            is_dir: false,
        });
    }

    // Also push directories for completeness (deduplicated)
    use std::collections::HashSet;
    let mut seen_dirs = HashSet::new();
    let mut extra_dirs = Vec::new();
    for e in &entries {
        if let Some(parent) = std::path::Path::new(&e.path).parent() {
            let mut acc = String::new();
            for component in parent.components() {
                if !acc.is_empty() {
                    acc.push('/');
                }
                acc.push_str(component.as_os_str().to_string_lossy().as_ref());
                if seen_dirs.insert(acc.clone()) {
                    extra_dirs.push(acc.clone());
                }
            }
        }
    }
    // Extend entries with collected directories
    for dir in extra_dirs {
        entries.push(ArchiveEntry { path: dir, size: 0, is_dir: true });
    }

    Ok(entries)
}

#[tauri::command]
pub fn native_drag_out_global(archive_path: String, file_paths: Vec<String>, _target_dir: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(first) = file_paths.first() {
            return tauri_plugin_dragout::macos::start_drag(&archive_path, first);
        }
        return Err("file_paths empty".into());
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("native drag-out not implemented for this platform".into())
    }
}
