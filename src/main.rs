//! Main entry point for the blitzarch CLI app

use blitzarch::cli::{self, Commands};
use blitzarch::{workers, extract};
use blitzarch::progress::ProgressState;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::io::{self, Write};
use std::time::Instant;

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
        Commands::Create { sharded: _, katana, inputs, output, level, workers, threads, codec_threads, memory_budget, password, progress, .. } => {
            if *katana {
                // Katana: new sharded MT format with optional progress
                let auto_threads = if *threads == 0 { num_cpus::get() } else { *threads };

                // parse memory budget
                let mem_budget_mb = cli::parse_memory_budget_mb(memory_budget)
                    .map_err(|e| format!("Invalid --memory-budget: {e}"))?;
                
                if *progress {
                    // Create progress callback for real-time CLI display
                    let progress_callback = create_cli_progress_callback("create");
                    blitzarch::katana::create_katana_archive_with_progress(
                        inputs,
                        output,
                        auto_threads,
                        *codec_threads,
                        mem_budget_mb,
                        password.clone(),
                        Some(progress_callback),
                    )?;
                } else {
                    // Use existing katana_stream for backward compatibility
                    blitzarch::katana_stream::create_katana_archive(
                        inputs,
                        output,
                        auto_threads,
                        *codec_threads,
                        *level,
                        password.clone(),
                    )?;
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

/// Creates a progress callback for CLI real-time display
fn create_cli_progress_callback(operation: &str) -> impl Fn(ProgressState) + Send + Sync + 'static {
    let operation = operation.to_string();
    let start_time = Instant::now();
    let last_update = Arc::new(Mutex::new(Instant::now()));
    
    move |state: ProgressState| {
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
        
        // Create progress bar
        let progress_width = 40;
        let filled = ((state.progress_percent / 100.0) * progress_width as f32) as usize;
        let empty = progress_width - filled;
        
        let progress_bar = format!(
            "[{}{}]", 
            "█".repeat(filled), 
            "░".repeat(empty)
        );
        
        // Calculate ETA
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
        let status_line = format!(
            "\r[{}] {} {:.1}% | {}/{} files | {:.1} MB/s | ETA: {}",
            operation.to_uppercase(),
            progress_bar,
            state.progress_percent,
            state.processed_files,
            state.total_files,
            state.speed_mbps,
            eta_str
        );
        
        // Print to stderr to avoid interfering with stdout
        eprint!("{}", status_line);
        io::stderr().flush().ok();
        
        // Final newline when completed
        if state.progress_percent >= 100.0 {
            eprintln!(); // New line after completion
        }
    }
}
