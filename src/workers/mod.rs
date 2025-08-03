//! Experimental worker-based compression module.

mod sharded;
pub use sharded::run_parallel_compression_sharded;

use crate::cli::{Commands, WorkerMode};

use crate::compress::{collect_file_metadata, group_files_into_bundles};
use crate::ArchiverError;

use crossbeam_channel::bounded;
use std::fs::File;
use std::sync::Arc;
use std::thread;

/// Parallel compression with heuristic (existing)
pub fn run_parallel_compression(args: Arc<Commands>, mode: WorkerMode) -> Result<(), ArchiverError> {
    if let Commands::Create { inputs, output, level, password, threads, text_bundle, use_lzma2, lz_level, adaptive, adaptive_threshold, .. } = &*args {
        let num_workers = match mode {
            WorkerMode::Auto => num_cpus::get(),
            WorkerMode::W2 => 2,
            WorkerMode::W4 => 4,
        };

        println!("Spawning {} worker threads.", num_workers);

        let mut metadata_list = collect_file_metadata(inputs)?;

        // --- Adaptive dataset-level decision ---
        let dense_ratio = {
            let total = metadata_list.iter().filter(|m| !m.is_dir).count();
            let dense = metadata_list.iter().filter(|m| m.dense_hint.unwrap_or(false)).count();
            if total == 0 { 0.0 } else { dense as f32 / total as f32 }
        };
        let mut global_algo = if *use_lzma2 {
            CompressionAlgo::Lzma2 { preset: lz_level.unwrap_or(6) }
        } else {
            CompressionAlgo::Zstd
        };
        if *adaptive && dense_ratio > 0.8 {
            println!("[adaptive] Dense dataset detected ({} % dense) → Store mode", (dense_ratio*100.0) as u32);
            global_algo = CompressionAlgo::Store;
        }

        let (directories, files): (Vec<_>, Vec<_>) = metadata_list.into_iter().partition(|m| m.is_dir);
        let bundles = group_files_into_bundles(&files, *text_bundle);

        let (bundle_sender, bundle_receiver) = bounded::<Box<[FileMetadata]>>(num_workers);
        
use tempfile::NamedTempFile;

use crate::compress::{create_store_temp_bundle, CompressionAlgo, compress_bundle_streaming};
use crate::archive::ArchiveWriter;
use crate::common::FileMetadata;
use std::path::PathBuf;

/// Message type sent from worker threads to writer thread.
enum WorkerBundle {
    Compressed {
        tmp_file: NamedTempFile,
        comp_size: u64,
        algo: CompressionAlgo,
        mapping: Vec<(PathBuf, u64, u64, u64)>,
    },
    Store {
        tmp_file: NamedTempFile,
        files: Vec<FileMetadata>,
    }
}

// channel of bundle results (compressed or store bundles)
let (compressed_sender, compressed_receiver) = bounded::<WorkerBundle>(num_workers);

        let scope_result = thread::scope(|s| {
            // --- Bundling/Compression Worker Threads ---
            for _ in 0..num_workers {
                let bundle_receiver = bundle_receiver.clone();
                let compressed_sender = compressed_sender.clone();
                let level = *level;
                let threads = *threads as u32;
                let _enable_pp = false;
                let _adaptive_flag = *adaptive;
                let threshold = *adaptive_threshold;
                let algo = global_algo;

                s.spawn(move || {
                    for bundle in bundle_receiver {
                            // --- Split bundle using cached dense_hint (no extra I/O) ---
                            let (dense_files, normal_files): (Vec<_>, Vec<_>) = bundle
                                .iter()
                                .cloned()
                                .partition(|m| m.dense_hint.unwrap_or(false));

                            let mut sub_bundles: Vec<(Vec<FileMetadata>, CompressionAlgo)> = Vec::new();
                            if !normal_files.is_empty() {
                                sub_bundles.push((normal_files, algo));
                            }
                            if !dense_files.is_empty() {
                                sub_bundles.push((dense_files, CompressionAlgo::Store));
                            }

                            for (sb_files, sb_algo) in sub_bundles {
                                 if sb_algo == CompressionAlgo::Store {
                                     // The writer thread now handles all encryption, so we just prepare a temp file.
                                     let (mut temp_file, stored_sizes) = match create_store_temp_bundle(&sb_files) {
                                         Ok(res) => res,
                                         Err(e) => {
                                             eprintln!("Worker store bundle preparation error: {e}");
                                             continue;
                                         }
                                     };

                                     if compressed_sender.send(WorkerBundle::Store { tmp_file: temp_file, files: sb_files }).is_err() { break; }
                                     continue;
                                 }
                                 let (mut temp_file, stored_sizes, used_algo, comp_size) = match compress_bundle_streaming(&sb_files, level, threads, None, false, false, sb_algo, threshold) {
                                    Ok(file) => file,
                                    Err(e) => {
                                        eprintln!("Worker compression error: {e}");
                                        continue;
                                    }
                                 };

                                 // build mapping for writer thread
                                 let mut mapping = Vec::new();
                                 let mut offset_cur = 0u64;
                                 for (idx, meta) in sb_files.iter().enumerate() {
                                     let st_sz = stored_sizes[idx];
                                     mapping.push((meta.path.clone(), offset_cur, st_sz, meta.size));
                                     offset_cur += st_sz;
                                 }
                                 if compressed_sender.send(WorkerBundle::Compressed { tmp_file: temp_file, comp_size, algo: used_algo, mapping }).is_err() {
                                    break;
                                }
                            }
                        }
                });
            }
            drop(compressed_sender);

            // --- Producer Thread (sends bundles to workers) ---
            s.spawn(move || {
                for bundle in bundles {
                    if bundle_sender.send(bundle.into_boxed_slice()).is_err() {
                        break;
                    }
                }
            });

            // --- Writer Thread (main thread) ---
            let output_file = File::create(output)?;
            let mut archive_writer = ArchiveWriter::new(output_file, password.clone(), global_algo)?;

            archive_writer.write_header()?;

            // Add directories to the index first.
            for dir_meta in directories {
                archive_writer.add_file_entry(dir_meta.path, true, 0, 0, 0, 0, Some(dir_meta.permissions));
            }

            let mut bundle_id_counter = 0;
            for bundle_msg in compressed_receiver {
                match bundle_msg {
                    WorkerBundle::Compressed { mut tmp_file, comp_size, algo: bundle_algo, mapping: original_paths } => {
                        let algo_str = match bundle_algo {
                            CompressionAlgo::Zstd => "zstd",
                            CompressionAlgo::Lzma2 { .. } => "lzma2",
                            CompressionAlgo::Store => "store",
                        };
                        archive_writer.set_current_algo(algo_str);
                        archive_writer.write_bundle_stream(&mut tmp_file, comp_size)?;

                        for (path, offset, stored_sz, uncomp_sz) in original_paths {
                            archive_writer.add_file_entry(path, false, bundle_id_counter, offset, stored_sz, uncomp_sz, None);
                        }
                        bundle_id_counter += 1;
                    },
                    WorkerBundle::Store { tmp_file, files } => {
                        archive_writer.write_store_bundle(tmp_file, &files)?;

                        let mut offset_in_bundle = 0;
                        for file in &files {
                            // The size of the file content + 8 bytes for the size prefix
                            let stored_size = file.size + 8;
                            archive_writer.add_file_entry(file.path.clone(), false, bundle_id_counter, offset_in_bundle, file.size, file.size, Some(file.permissions));
                            offset_in_bundle += stored_size;
                        }
                        bundle_id_counter += 1;
                    }
                }
            }

            archive_writer.finalize()
        });

        match scope_result {
            Ok(res) => Ok(res),
            Err(_) => Err(ArchiverError::Other("A worker thread panicked".into())),
        }

        } else {
        // This case should ideally be unreachable if called from main.rs
        Err(ArchiverError::Other("Incorrect command type passed to parallel compression".into()))
    }
}

// -----------------------------------------------------------------------------
// Legacy compatibility wrappers for CLI runner (temporary)
// -----------------------------------------------------------------------------
use std::path::{Path, PathBuf};
use crate::progress::ProgressState;

#[allow(clippy::too_many_arguments)]
pub fn create_archive_parallel(
    inputs: &[PathBuf],
    output: &PathBuf,
    _level: i32,
    threads: usize,
    codec_threads: u32,
    password: Option<&str>,
    _do_paranoid: bool,
    progress_cb: Option<Box<dyn Fn(ProgressState) + Send + Sync>>,
) -> Result<(), Box<dyn std::error::Error>> {
    crate::katana::create_katana_archive_with_progress(
        inputs,
        Path::new(output),
        threads,
        codec_threads,
        None,
        password.map(|s| s.to_string()),
        progress_cb,
    )
}

pub fn create_archive_single(
    inputs: &[PathBuf],
    output: &PathBuf,
    _level: i32,
    password: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    crate::katana::create_katana_archive_with_progress::<fn(ProgressState)>(
        inputs,
        Path::new(output),
        1,
        0,
        None,
        password.map(|s| s.to_string()),
        None,
    )
}

