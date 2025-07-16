//! # Deprecated Sharded Compression
//! 
//! This module implements the original, experimental, sharded parallel compression.
//! 
//! **Warning:** This implementation is considered deprecated and less efficient than the `Katana` format.
//! It is preserved for legacy purposes but should not be used for new development. The `Katana` (`--katana`)
//! implementation offers superior performance.
//! 
//! ## Strategy
//! 
//! 1. Files are grouped into fixed-size bundles.
//! 2. Each bundle is sent to a worker thread for compression into a temporary file.
//! 3. The main thread receives these temporary files and streams them into the final archive.

use crate::fsx::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use crossbeam_channel::bounded;
use tempfile::NamedTempFile;

use crate::archive::ArchiveWriter;
use crate::cli::{Commands, WorkerMode};
use crate::common::FileMetadata;
use crate::compress::{create_store_temp_bundle, collect_file_metadata, compress_bundle_streaming};
use crate::compress::CompressionAlgo;

use crate::ArchiverError;

/// Groups files into bundles by accumulating `target_size` bytes of *uncompressed*
/// data. A single file larger than the target size becomes its own bundle.
fn group_files_fixed_size(files: &[FileMetadata], target_size: u64) -> Vec<Vec<FileMetadata>> {
    let mut bundles = Vec::new();
    let mut current = Vec::new();
    let mut current_sz = 0u64;

    for f in files {
        if !current.is_empty() && current_sz + f.size > target_size {
            bundles.push(current);
            current = Vec::new();
            current_sz = 0;
        }
        current_sz += f.size;
        current.push(f.clone());
    }

    if !current.is_empty() {
        bundles.push(current);
    }
    bundles
}

/// `[DEPRECATED]` Entry point for the sharded parallel compression process.
///
/// This function is triggered by the `--sharded` CLI flag and manages the worker threads
/// for parallel bundle compression.
///
/// # Arguments
/// * `args` - The parsed command-line arguments.
/// * `mode` - The selected worker mode (e.g., Auto, W2, W4).
pub fn run_parallel_compression_sharded(args: Arc<Commands>, mode: WorkerMode) -> Result<(), ArchiverError> {
    // Destructure only the required fields from the command.
    let Commands::Create {
        inputs,
        output,
        level,
        password,

        threads,
        bundle_size,
        use_lzma2,
        lz_level,
        adaptive,
        adaptive_threshold,
        workers: _,     // handled by `mode`
        ..
    } = &*args else {
        return Err(ArchiverError::Other("Incorrect command variant for sharded compression".into()));
    };

    let num_workers = match mode {
        WorkerMode::Auto => num_cpus::get(),
        WorkerMode::W2 => 2,
        WorkerMode::W4 => 4,
    };
    println!("[sharded] Spawning {num_workers} worker threads (bundle_size {bundle_size} MiB)");

    // 1. Collect file metadata
    let mut metadata_list = collect_file_metadata(inputs)?;

    // Split directories vs regular files so we can add dirs to index immediately
    let (directories, files): (Vec<_>, Vec<_>) = metadata_list.into_iter().partition(|m| m.is_dir);

    // 2. Build bundles
    let target_bytes = *bundle_size as u64 * 1024 * 1024;
    let bundles = group_files_fixed_size(&files, target_bytes);

    // 3. Channels
    let (bundle_tx, bundle_rx) = bounded::<Box<[FileMetadata]>>(num_workers);

    /// Message type between workers and writer
    enum WorkerBundle {
        Compressed {
            tmp_file: NamedTempFile,
            comp_size: u64,
            algo: CompressionAlgo,
            mapping: Vec<(PathBuf, u64, u64, u64)>,
        },
        Store {
            tmp_file: NamedTempFile,
            mapping: Vec<(PathBuf, u64, u64, u64)>,
        },
    }
    let (result_tx, result_rx) = bounded::<WorkerBundle>(num_workers);

    // 4. Spawn scoped threads
    let scope_res = thread::scope(|s| {
        // --- Worker threads --------------------------------------------------
        for _ in 0..num_workers {
            let brx = bundle_rx.clone();
            let rtx = result_tx.clone();
            let lvl = *level;
            let th = *threads as u32;
            let enable_pp = false;
            let adaptive_flag = *adaptive;
            let threshold = *adaptive_threshold;
            let algo = if *use_lzma2 {
                CompressionAlgo::Lzma2 { preset: lz_level.unwrap_or(6) }
            } else {
                CompressionAlgo::Zstd
            };

            s.spawn(move || {
                for bundle in brx {
                    // Split into dense vs normal sub-bundles
                    let (dense_files, normal_files): (Vec<_>, Vec<_>) = bundle
                        .iter()
                        .cloned()
                        .partition(|m| crate::compress::is_dense_ext(m.path.extension().and_then(|s| s.to_str()).unwrap_or("")));

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
                                    eprintln!("[worker] store bundle preparation error: {e}");
                                    continue;
                                }
                            };

                            let mut mapping = Vec::new();
                            let mut offset_cur = 0u64;
                            for (idx, meta) in sb_files.iter().enumerate() {
                                let st_sz = stored_sizes[idx];
                                mapping.push((meta.path.clone(), offset_cur, st_sz, meta.size));
                                offset_cur += st_sz;
                            }

                            if rtx.send(WorkerBundle::Store { tmp_file: temp_file, mapping }).is_err() { break; }
                            continue;
                        }
                        let (mut tmp_file, stored_sizes, used_algo, comp_size) = match compress_bundle_streaming(&sb_files, lvl, th, None, false, false, sb_algo, threshold) {
                            Ok(t) => t,
                            Err(e) => {
                                eprintln!("[worker] compression error: {e}");
                                continue;
                            }
                        };

                        let mut mapping = Vec::new();
                        let mut offset = 0u64;
                        for (idx, meta) in sb_files.iter().enumerate() {
                            let st_sz = stored_sizes[idx];
                            mapping.push((meta.path.clone(), offset, st_sz, meta.size));
                            offset += st_sz;
                        }

                        if rtx.send(WorkerBundle::Compressed { tmp_file, comp_size, algo: used_algo, mapping }).is_err() {
                            break;
                        }
                    }
                }
            });
        }
        drop(result_tx); // so writer ends when workers finish

        // --- Producer thread -------------------------------------------------
        s.spawn(move || {
            for b in bundles {
                if bundle_tx.send(b.into_boxed_slice()).is_err() {
                    break;
                }
            }
        });

        // --- Writer thread (in the main scope) ------------------------------
        let algo_main = if *use_lzma2 {
            CompressionAlgo::Lzma2 { preset: lz_level.unwrap_or(6) }
        } else {
            CompressionAlgo::Zstd
        };
        let output_file = File::create(output)?;
        let mut writer = ArchiveWriter::new(output_file, password.clone(), algo_main)?;
        writer.write_header()?;

        // Write directory entries first so their bundle_id/offets are 0/0.
        for dir_meta in directories {
            writer.add_file_entry(dir_meta.path, true, 0, 0, 0, 0, Some(dir_meta.permissions));
        }

        let mut bundle_id = 0u32;
        for bundle_msg in result_rx {
            match bundle_msg {
                WorkerBundle::Compressed { mut tmp_file, comp_size, algo: bundle_algo, mapping } => {
                    let algo_str = match bundle_algo {
                        CompressionAlgo::Zstd => "zstd",
                        CompressionAlgo::Lzma2 { .. } => "lzma2",
                        CompressionAlgo::Store => "store",
                    };
                    writer.set_current_algo(algo_str);
                    writer.write_bundle_stream(&mut tmp_file, comp_size)?;

                    for (path, offset, stored_sz, uncomp_sz) in mapping {
                        writer.add_file_entry(path, false, bundle_id, offset, stored_sz, uncomp_sz, None);
                    }
                    bundle_id += 1;
                },
                WorkerBundle::Store { mut tmp_file, mapping } => {
                    writer.set_current_algo("store");
                    let total_size = mapping.iter().map(|(_, _, stored_sz, _)| *stored_sz).sum();
                    writer.write_bundle_stream(&mut tmp_file, total_size)?;

                    for (path, offset, stored_sz, uncomp_sz) in mapping {
                        writer.add_file_entry(path, false, bundle_id, offset, stored_sz, uncomp_sz, None);
                    }
                    bundle_id += 1;
                }
            }
        }

        writer.finalize()
    });

    scope_res.map_err(|_| ArchiverError::Other("A worker thread panicked (sharded)".into()))
}
