//! Reusable CLI runner so that both `blitzarch` (combined binary) and
//! the standalone `blitzarch-cli` can share the same implementation
//! without duplicating huge amounts of logic.
//!
//! The code is extracted from `src/main.rs::run_cli_app` unchanged.

use crate::cli::{self, Commands};
use crate::{workers, extract};
use crate::progress::ProgressState;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::io::{self, Write};
use std::time::Instant;
use std::sync::atomic::{AtomicBool, Ordering};
use term_size;

/// Public entry for running CLI logic. Mirrors old `run_cli_app`.
pub fn run_cli_app() -> Result<(), Box<dyn std::error::Error>> {
    let command = cli::run()?;

    match &command {
        Commands::Create { sharded: _, inputs, output, level, workers: worker_mode, threads, codec_threads, memory_budget, password, progress, skip_check, .. } => {
                // Katana: new sharded MT format with optional progress
                let do_paranoid = !*skip_check; // secure by default
                let auto_threads = if *threads == 0 { num_cpus::get() } else { *threads };

                // parse memory budget and export to env so katana_stream can read it
                let mem_budget_opt = cli::parse_memory_budget_mb(memory_budget)?;
                if let Some(mb) = mem_budget_opt {
                    std::env::set_var("BLITZARCH_MEMORY_MB", mb.to_string());
                }

                let pass = cli::get_password_from_opt_or_env(password.clone())?;

                // Construct progress callback if requested
                let progress_cb = if *progress {
                    Some(Box::new(create_cli_progress_callback("create")) as Box<dyn Fn(ProgressState) + Send + Sync>)
                } else { None };

                workers::create_archive_parallel(
                    inputs,
                    output,
                    *level,
                    auto_threads,
                    *codec_threads,
                    pass.as_deref(),
                    do_paranoid,
                    progress_cb,
                )?;

        }
        Commands::Extract { archive, files, output, password, strip_components, progress, .. } => {
                let pass = cli::get_password_from_opt_or_env(None)?;

                let progress_cb = if *progress {
                    Some(Box::new(create_cli_progress_callback("extract")) as Box<dyn Fn(ProgressState) + Send + Sync>)
                } else { None };

                extract::katana_extract(
                    archive,
                    files,
                    output,
                    *strip_components,
                    pass.as_deref(),
                    progress_cb,
                )?;

        }
        Commands::List { archive } => {
            let file = File::open(archive)?;
            extract::list_files(file)?;
        }
    }

    Ok(())
}

// --- utils for CLI progress -------------------------------------------------

fn create_cli_progress_callback(operation: &str) -> impl Fn(ProgressState) + Send + Sync + 'static {
    let operation = operation.to_string();
    let start_time = Instant::now();
    let last_update = Arc::new(Mutex::new(Instant::now()));
    let prev_len = Arc::new(Mutex::new(0usize));
    let done = Arc::new(AtomicBool::new(false));
    let done_cl = done.clone();

    move |state: ProgressState| {
        if done_cl.load(Ordering::Relaxed) {
            return;
        }
        let now = Instant::now();
        // Update every 100ms to avoid terminal spam, but always show 100% completion
        let should_update = state.progress_percent >= 100.0 || {
            let mut last = last_update.lock().unwrap();
            if now.duration_since(*last).as_millis() >= 100 {
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

        // Replace ETA placeholder after building line
        let mut bar_len = bar_width;
        let status_line = loop {
            let (mut line, _pb_len) = build_status_line(bar_len);
            line = line.replace("{ETA}", &eta_str);
            if line.len() <= term_width || bar_len <= 10 {
                break line;
            }
            if bar_len >= 4 {
                bar_len -= 4;
            } else {
                bar_len = 10;
            }
        };

        // Print to stderr to avoid interfering with stdout
        let mut line_to_print = status_line.clone();
        {
            let mut prev = prev_len.lock().unwrap();
            if *prev > line_to_print.len() {
                let diff = *prev - line_to_print.len();
                line_to_print.push_str(&" ".repeat(diff));
            }
            *prev = line_to_print.len();
        }
        eprint!("\r\x1B[2K{}", line_to_print);
        io::stderr().flush().ok();

        if state.progress_percent >= 100.0 {
            eprintln!();
            done_cl.store(true, Ordering::Relaxed);
        }
    }
}
