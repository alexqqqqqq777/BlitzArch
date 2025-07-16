//! Main entry point for the blitzarch CLI app

use blitzarch::cli::{self, Commands};
use blitzarch::{workers, extract};
use std::fs::File;
use std::sync::Arc;

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
        Commands::Create { sharded: _, katana, inputs, output, level, workers, threads, codec_threads, password, .. } => {
            if *katana {
                // Katana: new sharded MT format, ignore --level, always level 0 inside
                let auto_threads = if *threads == 0 { num_cpus::get() } else { *threads };
                blitzarch::katana_stream::create_katana_archive(inputs, output, auto_threads, *codec_threads, *level, password.clone())?;
            } else {
                workers::run_parallel_compression(Arc::new(command.clone()), *workers)?;
            }
        }
        Commands::Extract {
            archive,
            files,
            output,
            password,
        } => {
            if blitzarch::katana::is_katana_archive(archive)? {
                let out_dir = output.as_ref().ok_or("--output is required for Katana extract")?;
                let pass = cli::get_password_from_opt_or_env(password.clone())?;
                blitzarch::katana::extract_katana_archive_internal(archive, out_dir, files, pass)?;
            } else {
                let pass = cli::get_password_from_opt_or_env(password.clone())?;
                extract::extract_files(archive, files, pass.as_deref(), output.as_deref()).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            }
        }
        Commands::List { archive } => {
            let file = File::open(archive)?;
            extract::list_files(file).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        }
    }

    Ok(())
}
