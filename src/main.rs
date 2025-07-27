//! Main entry point for the blitzarch CLI app

use blitzarch::cli::{self, Commands};
use blitzarch::{workers, extract};
use blitzarch::progress::ProgressState;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::io::{self, Write};
use term_size;
use std::time::Instant;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() -> std::process::ExitCode {
    if let Err(e) = run_app() {
        if e.downcast_ref::<clap::Error>().is_none() {
            eprintln!("Error: {}", e);
        }
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    let command = cli::run()?;

    match &command {
        Commands::Create { sharded: _, katana, inputs, output, level: _, workers, threads, codec_threads, memory_budget, password, progress, skip_check, .. } => {
            if *katana {
                // Katana: new sharded MT format with optional progress
                let do_paranoid = !*skip_check; // secure by default
                let auto_threads = if *threads == 0 { num_cpus::get() } else { *threads };

                // parse memory budget and export to env so katana_stream can read it
                let mem_budget_mb = cli::parse_memory_budget_mb(memory_budget)
                    .map_err(|e| format!("Invalid --memory-budget: {e}"))?;
                if let Some(mb) = mem_budget_mb {
                    std::env::set_var("BLITZ_MEM_BUDGET_MB", mb.to_string());
                }
                if *progress {
                    // Create progress callback for real-time CLI display
                    let progress_callback = create_cli_progress_callback("create");
                    blitzarch::katana_stream::create_katana_archive_with_progress(
                        inputs,
                        output,
                        auto_threads,
                        *codec_threads,
                        mem_budget_mb,
                        password.clone(),
                        None, // compression_level - use AutoTune default
                        !do_paranoid, // skip_check - invert paranoid flag
                        Some(progress_callback),
                    )?;

                    // Paranoid BLAKE3 verification
                    if do_paranoid {
                        perform_paranoid_check(output)?;
                    }
                } else {
                    // Use existing katana_stream for backward compatibility
                    blitzarch::katana_stream::create_katana_archive(
                        inputs,
                        output,
                        auto_threads,
                        *codec_threads,
                        mem_budget_mb,
                        password.clone(),
                        None, // compression_level - use AutoTune default
                        None::<fn(blitzarch::progress::ProgressState)>, // no progress callback for CLI
                    )?;
                    if do_paranoid {
                        perform_paranoid_check(output)?;
                    }
                }
            } else {
                workers::run_parallel_compression(Arc::new(command.clone()), *workers)?;
            }
        }
        Commands::Extract {
            archive,
            files,
            output,
            password,
            strip_components,
            progress,
        } => {
            if blitzarch::katana::is_katana_archive(archive)? {
                let out_dir = output.as_ref().ok_or("--output is required for Katana extract")?;
                let pass = cli::get_password_from_opt_or_env(password.clone())?;
                
                if *progress {
                    // Create progress callback for real-time CLI display
                    let progress_callback = create_cli_progress_callback("extract");
                    blitzarch::katana::extract_katana_archive_with_progress(
                        archive, out_dir, files, pass, *strip_components, Some(progress_callback)
                    )?;
                } else {
                    blitzarch::katana::extract_katana_archive_internal(archive, out_dir, files, pass, *strip_components)?;
                }
            } else {
                let pass = cli::get_password_from_opt_or_env(password.clone())?;
                extract::extract_files(archive, files, pass.as_deref(), output.as_deref(), *strip_components).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            }
        }
        Commands::List { archive } => {
            let file = File::open(archive)?;
            extract::list_files(file).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        }
    }

    Ok(())
}

// -----------------------------------------------------------------------------
/// Reads the file twice and compares BLAKE3-256 digests; returns Err on mismatch.
fn perform_paranoid_check(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;
    fn hash_file(p: &std::path::Path) -> Result<blake3::Hash, std::io::Error> {
        let mut f = std::fs::File::open(p)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
        }
        Ok(hasher.finalize())
    }
    let h1 = hash_file(path)?;
    // ensure fs flush already done by caller
    let h2 = hash_file(path)?;
    if h1 != h2 {
        let _ = std::fs::remove_file(path);
        return Err("Paranoid integrity check failed: BLAKE3 mismatch".into());
    }
    println!("[paranoid] Integrity verified, BLAKE3 = {}", h1.to_hex());
    Ok(())
}

/// Creates a progress callback for CLI real-time display
fn create_cli_progress_callback(operation: &str) -> impl Fn(ProgressState) + Send + Sync + 'static {
    let operation = operation.to_string();
    let start_time = Instant::now();
    let last_update = Arc::new(Mutex::new(Instant::now()));
    let prev_len = Arc::new(Mutex::new(0usize));
    let done = Arc::new(AtomicBool::new(false));
    let done_cl = done.clone();
    
    move |state: ProgressState| {
        if done_cl.load(Ordering::Relaxed) { return; }
        let now = Instant::now();
        // Update every 100ms to avoid terminal spam, but always show 100% completion
        let should_update = state.progress_percent >= 100.0 || {
            let mut last = last_update.lock().unwrap();
            if now.duration_since(*last).as_millis() >= 50 {
                *last = now;
                true
            } else {
                false
            }
        };
        
        if !should_update {
            return;
        }
        
        // Determine terminal width (default 80)
        let term_width = term_size::dimensions().map(|(w, _)| w as usize).unwrap_or(80);

        // Initial progress bar width cap
        let bar_width: usize = 40;
        // Recompute bar widths later if we need to shrink

        // Helper to build status line for given bar width
        let build_status_line = |bw: usize| -> (String, usize) {
            let filled = ((state.progress_percent / 100.0) * bw as f32) as usize;
            let empty = bw - filled;
            let progress_bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));
            let line = format!(
                "[{}] {} {:.1}% | {}/{} files | {:.1} MB/s | ETA: {}",
                operation.to_uppercase(),
                progress_bar,
                state.progress_percent,
                state.processed_files,
                state.total_files,
                state.speed_mbps,
                "{ETA}" // placeholder, will replace below
            );
            (line, progress_bar.len())
        };
        let elapsed = start_time.elapsed().as_secs_f32();
        let eta_str = if state.speed_mbps > 0.0 && state.progress_percent > 0.0 {
            let remaining_percent = 100.0 - state.progress_percent;
            let eta_seconds = (elapsed * remaining_percent) / state.progress_percent;
            if eta_seconds > 60.0 {
                format!("{:.1}m", eta_seconds / 60.0)
            } else {
                format!("{:.1}s", eta_seconds)
            }
        } else {
            "--".to_string()
        };
        
        // Format output
        // Calculate ETA string first
        let eta_final = eta_str;

        // Build status line and shrink bar if too long
        let mut bar_len = bar_width;
        let status_line = loop {
            let (mut line, _pb_len) = build_status_line(bar_len);
            // replace placeholder ETA
            line = line.replace("{ETA}", &eta_final);
            if line.len() <= term_width || bar_len <= 10 {
                break line;
            }
            // shrink bar and retry
            if bar_len >= 4 { bar_len -= 4; } else { bar_len = 10; }
        };
        
        // Print to stderr to avoid interfering with stdout
        // Pad with spaces if new line is shorter than previous to fully overwrite
        let mut line_to_print = status_line.clone();
        {
            let mut prev = prev_len.lock().unwrap();
            if *prev > line_to_print.len() {
                let diff = *prev - line_to_print.len();
                line_to_print.push_str(&" ".repeat(diff));
            }
            *prev = line_to_print.len();
        }
        // Clear line + carriage return, then print padded string
        eprint!("\r\x1B[2K{}", line_to_print);
        io::stderr().flush().ok();
        
        // Final newline when completed
        if state.progress_percent >= 100.0 {
            eprintln!(); // New line after completion
            done_cl.store(true, Ordering::Relaxed);
        }
    }
}
