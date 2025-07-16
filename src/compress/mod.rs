//! # Compression Pipeline
//! 
//! This module implements the core compression logic for `blitzarch`. It orchestrates the entire
//! process, from discovering files to writing compressed data bundles into an archive.
//! 
//! ## Key Features:
//! - **File Discovery**: Recursively finds files in input directories.
//! - **Bundling**: Groups small files into larger bundles to improve compression ratios.
//! - **Dictionary Training**: Can create a shared `zstd` dictionary from file samples to further improve compression of small, similar files.
//! - **Adaptive Compression**: Can automatically switch to `Store` mode for incompressible data, saving CPU time.

use crate::archive::ArchiveWriter;
use crate::common::FileMetadata;
use crate::ArchiverError;

use jwalk;
use std::io::Write;
use std::fs::{self, File};
use std::io::{self, Read, Seek};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt; // mode() helper
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tempfile::NamedTempFile;

/// Defines the available compression algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgo {
    /// Use the Zstandard algorithm. Fast and effective.
    Zstd,
    /// Plain storage without any compression. Useful for already-compressed ("dense") data.
    Store,
    /// Use the LZMA2 algorithm with a given preset (0-9).
    Lzma2 { preset: u32 },
}

/// Holds all configuration options for a compression operation.
#[derive(Debug, Clone)]
pub struct CompressOptions {
    /// The compression level (e.g., for Zstd or LZMA2).
    pub level: i32,
    /// The number of threads to use for compression codecs that support it.
    pub threads: u32,
    /// The strategy for bundling text files.
    pub text_bundle: TextBundleMode,
    /// Whether to enable adaptive compression (switching to `Store` for incompressible data).
    pub adaptive: bool,
    /// The threshold for adaptive compression.
    pub adaptive_threshold: f64,
    /// The primary compression algorithm to use.
    pub algo: CompressionAlgo,
}

// A simple bin-packing strategy: group files until a certain size is reached.
use crate::cli::TextBundleMode;


const BUNDLE_SIZE_SMALL: u64 = 16 * 1024 * 1024; // 16 MiB
const BUNDLE_SIZE_AUTO:  u64 = 64 * 1024 * 1024; // 64 MiB
const BUNDLE_SIZE_WINDOW: u64 = 128 * 1024 * 1024; // 128 MiB

/// Returns true if the file extension is typically already compressed / dense.
pub(crate) fn is_dense_ext(ext: &str) -> bool {
    matches!(ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "mp4" | "mkv" | "mp3" | "ogg" | "flac" |
        "zip" | "rar" | "7z" | "gz" | "bz2" | "xz" | "pdf" | "docx" | "pptx" | "xlsx")
}

/// Quick magic-bytes detection for already-compressed formats.
/// This is *not* a complete parser; we just peek first few bytes to flag dense data.
fn is_dense_magic(path: &std::path::Path) -> bool {
    use std::fs::File;
    use std::io::Read;
    let mut buf = [0u8; 8];
    let Ok(mut f) = File::open(path) else { return false };
    let Ok(n) = f.read(&mut buf) else { return false };
    let slice = &buf[..n];
    match slice {
        b if b.starts_with(b"\x89PNG") => true,                     // PNG
        b if b.starts_with(b"\xFF\xD8") => true,                    // JPEG
        b if b.starts_with(b"\x1F\x8B") => true,                    // GZIP
        b if b.starts_with(b"PK\x03\x04") => true,                  // ZIP/Docx/etc.
        b if b.starts_with(b"7z\xBC\xAF\x27\x1C") => true,        // 7z
        b if b.starts_with(b"Rar!") => true,                         // RAR
        b if b.starts_with(b"%PDF") => true,                         // PDF
        _ => false,
    }
}

/// Quick heuristic to decide whether a file is text-like and worth preprocessing.
/// 1. Cheap extension check for common source/text formats.
/// 2. If extension unknown, sample first bytes and count printable ASCII ratio.
///    If ≥85 % printable, treat as text.
fn should_preprocess(path: &std::path::Path, sample: &[u8]) -> bool {
    // Fast path: extension whitelist
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        match ext.to_ascii_lowercase().as_str() {
            "rs" | "c" | "h" | "cpp" | "hpp" | "json" | "txt" | "md" | "toml" | "yaml" | "yml" | "html" | "htm" | "css" | "js" | "ts" | "py" | "csv" => {
                return true;
            }
            // Obvious binary extensions – immediately skip
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "mp4" | "mkv" | "pdf" | "zip" | "gz" | "xz" | "tar" | "rar" | "7z" | "iso" => {
                return false;
            }
            _ => {}
        }
    }

    // Fallback: printable ratio on first bytes
    if sample.is_empty() {
        return false;
    }
    let printable = sample
        .iter()
        .filter(|b| matches!(b, 0x09 | 0x0A | 0x0D | 0x20..=0x7E))
        .count();
    (printable as f32 / sample.len() as f32) >= 0.85
}

// Main entry point for single-threaded compression.
/// Runs the entire compression process from file discovery to final archive generation.
///
/// This is the main entry point for creating an archive. It orchestrates the following steps:
/// 1. Collects metadata for all input files and directories.
/// 2. Determines the best compression strategy (e.g., adaptive mode).
/// 3. Trains a shared compression dictionary from file samples if beneficial.
/// 4. Groups files into bundles to optimize compression ratios.
/// 5. Compresses each bundle and writes it to the archive.
/// 6. Writes all necessary metadata, including the file index and footer, to finalize the archive.
/// Creates a temporary file containing the raw, concatenated data of a set of files.
/// This is used for `Store` mode, where data is not compressed but still needs to be
/// passed through the same encryption and writing pipeline as compressed data.
pub fn create_store_temp_bundle(
    files: &[FileMetadata],
) -> Result<(NamedTempFile, Vec<u64>), ArchiverError> {
    let mut temp_file = NamedTempFile::new().map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;
    let mut stored_sizes = Vec::with_capacity(files.len());

    for file_meta in files {
        let mut input_file = File::open(&file_meta.absolute_path).map_err(|e| ArchiverError::Io {
            source: e,
            path: file_meta.absolute_path.clone(),
        })?;
        // Prepend the data with its size
        let file_size = input_file.metadata().map(|m| m.len()).unwrap_or(0);
        temp_file.write_all(&file_size.to_le_bytes()).map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;

        let written = io::copy(&mut input_file, &mut temp_file).map_err(|e| ArchiverError::Io {
            source: e,
            path: file_meta.absolute_path.clone(),
        })?;
        stored_sizes.push(8 + written);
    }

    temp_file.seek(io::SeekFrom::Start(0)).map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;
    Ok((temp_file, stored_sizes))
}

pub fn run(
    inputs: &[PathBuf],
    output: &PathBuf,
    options: CompressOptions,
    password: Option<String>,
) -> Result<(), ArchiverError> {
    let mut metadata_list = collect_file_metadata(inputs)?;


    // --- Adaptive selection: if majority of files are "dense" (already compressed media/archives), switch to Store ---
    let dense_ratio = {
        let mut dense = 0usize;
        let mut total = 0usize;
        for m in &metadata_list {
            if m.is_dir { continue; }
            total += 1;
            if is_dense_ext(m.path.extension().and_then(|s| s.to_str()).unwrap_or("")) {
                dense += 1;
            }
        }
        if total == 0 { 0.0 } else { dense as f32 / total as f32 }
    };
    let mut selected_algo = options.algo;
    if options.adaptive && (dense_ratio as f64) > options.adaptive_threshold {
        selected_algo = CompressionAlgo::Store;
        println!("[adaptive] Detected dense dataset ({} % dense) → using plain Store mode", (dense_ratio*100.0) as u32);
    }

    let (directories, files): (Vec<_>, Vec<_>) = metadata_list.into_iter().partition(|m| m.is_dir);

    // Train dictionary on file samples
    let dictionary = train_dictionary(&files)?;

    let bundles = group_files_into_bundles(&files, options.text_bundle);

    let output_file = File::create(output).map_err(|e| ArchiverError::Io { source: e, path: output.clone() })?;
    let mut archive_writer = ArchiveWriter::new(output_file, password, selected_algo)?;
    archive_writer.write_header()?;

    // Write dictionary to the archive if one was created.
    if let Some(ref dict_data) = dictionary {
        archive_writer.write_dictionary(dict_data)?;
    }

    // Add directories to the index first.
    for dir_meta in directories {
        archive_writer.add_file_entry(dir_meta.path, true, 0, 0, 0, 0, Some(dir_meta.permissions));
    }

    let mut bundle_id_counter = 0u32; // tracks logical bundles after splitting
    for bundle in bundles {
        // --- Split bundle into dense vs normal sub-bundles (per-file adaptive) ---
        let (dense_files, normal_files): (Vec<_>, Vec<_>) = bundle
            .iter()
            .cloned()
            .partition(|m| is_dense_ext(m.path.extension().and_then(|s| s.to_str()).unwrap_or("")));

        let mut sub_bundles: Vec<(Vec<FileMetadata>, CompressionAlgo)> = Vec::new();
        if !normal_files.is_empty() {
            sub_bundles.push((normal_files, selected_algo));
        }
        if !dense_files.is_empty() {
            sub_bundles.push((dense_files, CompressionAlgo::Store));
        }

        for (sb_files, sb_algo) in sub_bundles {
        let (mut maybe_temp, processed_sizes, used_algo) = if sb_algo == CompressionAlgo::Store {
            // --- Direct store path, no compression ---
            let (temp_file, sizes) = create_store_temp_bundle(&sb_files)?;
            (Some(temp_file), sizes, CompressionAlgo::Store)
        } else {
            let (tmp_file, sizes, used, _comp_size) = compress_bundle_streaming(
              &sb_files,
             options.level,
             options.threads,
             dictionary.as_deref(),
             /*preprocess already handled*/ false,
             /*adaptive already handled*/ false,
             sb_algo,
             options.adaptive_threshold,
        )?;
            (Some(tmp_file), sizes, used)
        };

        if let Some(mut temp_file) = maybe_temp {
            let mut buffer = Vec::new();
            temp_file.read_to_end(&mut buffer)
                .map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;
            // tag bundle with its own algorithm
            let algo_str = match used_algo {
                CompressionAlgo::Zstd => "zstd",
                CompressionAlgo::Lzma2 { .. } => "lzma2",
                CompressionAlgo::Store => "store",
            };
            archive_writer.set_current_algo(algo_str);
            archive_writer.write_bundle(&buffer)?;
        }

        // Add file index entries for both store and compressed paths
        let mut offset_in_bundle = 0u64;
        for (idx, file_meta) in sb_files.iter().enumerate() {
            let stored_size = processed_sizes[idx];
            archive_writer.add_file_entry(
                file_meta.path.clone(),
                false,
                bundle_id_counter,
                offset_in_bundle,
                stored_size,
                file_meta.size, // original uncompressed size
                Some(file_meta.permissions),
            );
            offset_in_bundle += stored_size;
        }
        bundle_id_counter += 1;
    } // end sub_bundles loop
    } // end for bundle

    archive_writer.finalize()?;
    Ok(())
}

pub fn collect_file_metadata(paths: &[PathBuf]) -> Result<Vec<FileMetadata>, ArchiverError> {
    let mut metadata_list = Vec::new();

    for path_arg in paths {
        let base_path = if path_arg.is_dir() {
            path_arg.clone()
        } else {
            path_arg.parent().unwrap_or(path_arg).to_path_buf()
        };
        let absolute_base_path = fs::canonicalize(&base_path)
            .map_err(|e| ArchiverError::Io { source: e, path: base_path.clone() })?;

        for entry in jwalk::WalkDir::new(path_arg).sort(false) {
            let entry = entry.map_err(|e| ArchiverError::Io { source: e.into(), path: path_arg.clone() })?;
            let path = entry.path();

            let metadata = match fs::symlink_metadata(&path) {
                Ok(md) => md,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(ArchiverError::Io { source: e, path: path.clone() }),
            };

            if metadata.file_type().is_symlink() {
                continue;
            }

            let absolute_path = fs::canonicalize(&path)
                .map_err(|e| ArchiverError::Io { source: e, path: path.clone() })?;

            let relative_path = absolute_path
                .strip_prefix(&absolute_base_path)
                .map_err(|_e| ArchiverError::StripPrefix {
                    prefix: absolute_base_path.clone(),
                    path: absolute_path.clone(),
                })?
                .to_path_buf();

            let permissions: u32 = {
                #[cfg(unix)]
                { metadata.permissions().mode() }
                #[cfg(not(unix))]
                { 0 }
            };
            let modified_time = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs();

            let ext_dense = relative_path
                .extension()
                .and_then(|s| s.to_str())
                .map(is_dense_ext)
                .unwrap_or(false);
            let dense = if ext_dense {
                true
            } else {
                // Perform a quick magic-byte probe once; result cached in dense_hint
                is_dense_magic(&absolute_path)
            };
            let file_meta = FileMetadata {
                absolute_path,
                path: relative_path,
                size: metadata.len(),
                permissions,
                modified_time,
                is_dir: metadata.is_dir(),
                dense_hint: Some(dense),
            };
            metadata_list.push(file_meta);
        }
    }
    Ok(metadata_list)
}

// Constants for dictionary training
const DICTIONARY_MAX_SIZE: usize = 128 * 1024; // 128 KiB
const SAMPLE_SIZE: usize = 8 * 1024; // 8 KiB sample from each file

/// Trains a zstd compression dictionary from a sample of files.
///
/// The strategy is to take a small sample from the beginning of many files to create a
/// shared dictionary. This is highly effective for datasets containing many small, similar files
/// (e.g., text, JSON, code), as the dictionary can capture common patterns and significantly
/// improve the compression ratio.
///
/// # Arguments
/// * `files` - A slice of `FileMetadata` for the files to be sampled.
///
/// # Returns
/// * `Ok(Some(Vec<u8>))` containing the dictionary data if training was successful.
/// * `Ok(None)` if there are too few files to warrant a dictionary.
/// * `Err(ArchiverError)` if an I/O error occurs.
pub fn train_dictionary(files: &[FileMetadata]) -> Result<Option<Vec<u8>>, ArchiverError> {
    if files.len() < 10 { // Don't bother for very few files
        return Ok(None);
    }

    let mut samples = Vec::new();
    let mut total_samples_size = 0;

    for file_meta in files.iter().filter(|f| f.size > 0) {
        let file = File::open(&file_meta.absolute_path).map_err(|e| ArchiverError::Io {
            source: e,
            path: file_meta.absolute_path.clone(),
        })?;
        let mut sample = Vec::with_capacity(SAMPLE_SIZE);
        let bytes_read = file.take(SAMPLE_SIZE as u64).read_to_end(&mut sample)
            .map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })?;
        
        if bytes_read > 0 {
            samples.push(sample);
            total_samples_size += bytes_read;
        }

        // Stop if we have enough samples to train a good dictionary
        if total_samples_size > DICTIONARY_MAX_SIZE * 100 {
            break;
        }
    }

    if samples.is_empty() {
        return Ok(None);
    }

    let dict_data = zstd::dict::from_samples(&samples, DICTIONARY_MAX_SIZE)
        .map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;

    println!("Trained a dictionary of size: {} bytes", dict_data.len());
    Ok(Some(dict_data))
}

use crate::dict_cache;

pub fn compress_bundle_streaming(
    files: &[FileMetadata],
    level: i32,
    threads: u32,
    dictionary: Option<&[u8]>,
    enable_preprocess: bool,
    adaptive: bool,
    global_algo: CompressionAlgo,
    adaptive_threshold: f64,
) -> Result<(NamedTempFile, Vec<u64>, CompressionAlgo, u64), ArchiverError> {
    // Determine bundle-specific algorithm if adaptive is enabled
    // --- Adaptive Store decision (improved) ---
    // Switch to `CompressionAlgo::Store` if at least `adaptive_threshold` fraction of *bytes*
    // in the bundle belong to files that are already dense (e.g. PNG/ZIP/JPEG).
    // 1. We short-circuit if global algorithm is already `Store`.
    // 2. We first check the file extension and only fall back to magic-byte sniffing
    //    if extension is not in the dense whitelist to minimise extra I/O.
    let mut algo = global_algo;
    // Apply adaptive Store switch only for low compression levels (L1/L2) where speed is priority.
    if adaptive && level < 3 && !matches!(global_algo, CompressionAlgo::Store) {
        let mut dense_bytes: u64 = 0;
        let mut total_bytes: u64 = 0;
        for file_meta in files {
            if file_meta.size == 0 {
                // Skip empty files – they do not influence the decision.
                continue;
            }
            total_bytes += file_meta.size;
            let mut is_dense = if let Some(flag) = file_meta.dense_hint {
                 flag
             } else {
                 let ext_dense = file_meta
                     .path
                     .extension()
                     .and_then(|s| s.to_str())
                     .map(is_dense_ext)
                     .unwrap_or(false);
                 if ext_dense {
                     true
                 } else {
                     // Fallback to cheap magic-byte sniffing (first few bytes).
                     is_dense_magic(&file_meta.absolute_path)
                 }
             };
            if is_dense {
                dense_bytes += file_meta.size;
            }
        }
        if total_bytes > 0 {
            let dense_ratio = dense_bytes as f64 / total_bytes as f64;
            if dense_ratio > adaptive_threshold {
                algo = CompressionAlgo::Store;
                // Optional: print diagnostic to help tuning
                println!(
                    "[adaptive] Dense bundle detected ({:.2}% dense by bytes) → Store",
                    dense_ratio * 100.0
                );
            }
        }
    }


    let mut temp_file = NamedTempFile::new().map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;
    let mut stored_sizes: Vec<u64> = Vec::with_capacity(files.len());

    // Initialise global dictionary cache (decompression side)
    if let Some(dict_bytes) = dictionary {
        dict_cache::init(dict_bytes.to_vec().into_boxed_slice());
    }

    match algo {
            CompressionAlgo::Zstd => {
                use std::io::{Read, Write, Seek, SeekFrom};
                // Prepare dictionary once (if provided) to avoid rebuilding per file
                let prepared_dict = dictionary.map(|d| zstd::dict::EncoderDictionary::copy(d, level));

                // --- Per-file encoder (simpler borrow semantics) -------------------------
                for file_meta in files {
                    let start_pos = temp_file.as_file_mut().seek(SeekFrom::End(0))?;

                    // Create a fresh encoder but reuse prepared dictionary (cheap)
                    let mut encoder = if let Some(ref dict) = prepared_dict {
                        zstd::stream::Encoder::with_prepared_dictionary(&mut temp_file, dict)
                    } else {
                        zstd::stream::Encoder::new(&mut temp_file, level)
                    }
                    .map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;

                    encoder
                        .include_checksum(false) // disable CRC per frame
                        .map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;
                    encoder
                        .multithread(threads)
                        .map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;

                    // --- Write file data ------------------------------------------------
                    let mut file = File::open(&file_meta.absolute_path).map_err(|e| ArchiverError::Io {
                        source: e,
                        path: file_meta.absolute_path.clone(),
                    })?;
                    let mut first_bytes = [0u8; 4096];
                    let n_peek = file.read(&mut first_bytes).map_err(|e| ArchiverError::Io {
                        source: e,
                        path: file_meta.absolute_path.clone(),
                    })?;

                    encoder.write_all(&u32::MAX.to_le_bytes()).map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })?;
                    encoder.write_all(&first_bytes[..n_peek]).map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })?;
                    std::io::copy(&mut file, &mut encoder).map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })?;
                    encoder.finish().map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;

                    let end_pos = temp_file.as_file_mut().seek(SeekFrom::End(0))?;
                    stored_sizes.push(end_pos - start_pos);
                }
            },
            CompressionAlgo::Lzma2 { preset } => {
                use std::io::{Read, Write, Seek, SeekFrom};
                use xz2::stream::{MtStreamBuilder, Check};
                let lz_threads = if threads == 0 { std::cmp::max(1, num_cpus::get() as u32) } else { threads };
                let mut builder = MtStreamBuilder::new();
                builder.threads(lz_threads).preset(preset).check(Check::Crc64);

                for file_meta in files {
                    let start_pos = temp_file.as_file_mut().seek(SeekFrom::End(0))?;

                    let stream = builder
                        .encoder()
                        .map_err(|e| ArchiverError::Io { source: std::io::Error::new(std::io::ErrorKind::Other, e), path: PathBuf::new() })?;
                    let mut encoder = xz2::write::XzEncoder::new_stream(&mut temp_file, stream);

                    // ----- Encode single file -----
                    let mut file = File::open(&file_meta.absolute_path).map_err(|e| ArchiverError::Io {
                        source: e,
                        path: file_meta.absolute_path.clone(),
                    })?;
                    let mut first_bytes = [0u8; 4096];
                    let n_peek = file.read(&mut first_bytes).map_err(|e| ArchiverError::Io {
                        source: e,
                        path: file_meta.absolute_path.clone(),
                    })?;
                    encoder.write_all(&u32::MAX.to_le_bytes()).map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })?;
                    encoder.write_all(&first_bytes[..n_peek]).map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })?;
                    std::io::copy(&mut file, &mut encoder).map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })?;
                    encoder.finish().map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;

                    let end_pos = temp_file.as_file_mut().seek(SeekFrom::End(0))?;
                    stored_sizes.push(end_pos - start_pos);
                }
            },
            CompressionAlgo::Store => {
                // No additional compression; write files directly.
                let mut writer = &mut temp_file;
                write_files_to_encoder(&mut writer, files, enable_preprocess, &mut stored_sizes)?;
            }
        }

    // Determine compressed size once
    let comp_size = temp_file.as_file_mut().seek(io::SeekFrom::End(0)).map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;
    temp_file.as_file_mut()
        .seek(io::SeekFrom::Start(0))
        .map_err(|e| ArchiverError::Io { source: e, path: PathBuf::new() })?;

    Ok((temp_file, stored_sizes, algo, comp_size))
}

fn write_files_to_encoder<W: Write>(
    encoder: &mut W,
    files: &[FileMetadata],
    enable_preprocess: bool,
    stored_sizes: &mut Vec<u64>,
) -> Result<(), ArchiverError> {
    use std::io::{self, Read, Write};

    for file_meta in files {
        // Open file for peeking and streaming
        let mut file = File::open(&file_meta.absolute_path).map_err(|e| ArchiverError::Io {
            source: e,
            path: file_meta.absolute_path.clone(),
        })?;

        // Peek first 4 KiB to decide preprocessing
        let mut first_bytes = [0u8; 4096];
        let n_peek = file.read(&mut first_bytes).map_err(|e| ArchiverError::Io {
            source: e,
            path: file_meta.absolute_path.clone(),
        })?;

        // --- Store mode uses 8-byte length prefix followed by raw data ---
        let file_size = file_meta.size;
        encoder.write_all(&file_size.to_le_bytes()).map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })?;
        let copied_head = encoder.write(&first_bytes[..n_peek]).map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })? as u64;
        let copied_tail = io::copy(&mut file, encoder).map_err(|e| ArchiverError::Io { source: e, path: file_meta.absolute_path.clone() })?;
        stored_sizes.push(8 + copied_head + copied_tail);
    }

    Ok(())
}


pub fn group_files_into_bundles(metadata_list: &[FileMetadata], mode: TextBundleMode) -> Vec<Vec<FileMetadata>> {
    let mut bundles = Vec::new();
    if metadata_list.is_empty() {
        return bundles;
    }

    let mut current_bundle = Vec::new();
    let mut current_limit = BUNDLE_SIZE_SMALL;
    let mut current_bundle_size = 0;
    let mut current_ext = metadata_list[0]
        .path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    for metadata in metadata_list {
        let metadata_ext = metadata
            .path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if !current_bundle.is_empty()
            && (current_bundle_size + metadata.size > current_limit
                || metadata_ext != current_ext)
        {
            bundles.push(current_bundle);
            current_bundle = Vec::new();
            current_bundle_size = 0;
        }

        if current_bundle.is_empty() {
            current_ext = metadata_ext.clone();
            // decide limit based on extension heuristic
            let is_text = matches!(current_ext.as_str(),
                "txt" | "csv" | "md" | "json" | "xml" | "html" | "htm" | "js" | "css" | "rs" | "py" | "java" | "go" | "kt" | "c" | "cpp" | "h" | "hpp" | "ts" | "tsx" | "yaml" | "yml"
            );
            current_limit = if is_text {
                match mode {
                    TextBundleMode::Small => BUNDLE_SIZE_SMALL,
                    TextBundleMode::Auto => BUNDLE_SIZE_AUTO,
                    TextBundleMode::Window => BUNDLE_SIZE_WINDOW,
                }
            } else {
                BUNDLE_SIZE_SMALL
            };
        }

        current_bundle_size += metadata.size;
        current_bundle.push(metadata.clone());
    }

    if !current_bundle.is_empty() {
        bundles.push(current_bundle);
    }

    bundles
}
