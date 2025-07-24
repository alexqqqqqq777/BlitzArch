//! Parallel extraction helpers (initial scaffolding).
//! 
//! This module introduces a foundation for true multi-threaded extraction. The current
//! implementation keeps behaviour identical to the legacy single-thread path, but places
//! the logic behind an API that can later be parallelised.
//!
//! Rationale: the existing `extract_files` already parallelises *across* bundles with
//! Rayon, but a typical Katana archive may contain only one bundle (or a handful), thus
//! CPU utilisation during decompression is still low.  The goal is to further split work
//! *within* a bundle so multiple cores stay busy even for a single large solid stream.
//!
//! For the first step we provide a thin wrapper that re-uses the legacy sequential code
//! but behind a common interface.  Subsequent commits will replace the internals with a
//! proper worker pool that:
//! • maintains a dedicated decoder per thread (or shared ring-buffer for zstd)
//! • copies / writes different file slices in parallel
//! • honours encrypted bundles by falling back to single-threaded path.
//!
//! This staged approach keeps the code compiling and allows incremental benchmarking.

use std::fs::{self, File};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::fs::Permissions;
use crate::extract::block_pipeline;
use std::io::{self, Read, Seek, SeekFrom, Write};
use crate::extract::writer_pool;
use std::io::BufReader;

use std::path::{Path};

use crate::archive::{ArchiveIndex, FileIndexEntry};

/// Temporary re-export of helper until we refactor it out of `mod.rs`.
fn extract_from_decoder(
    decoder: &mut dyn Read,
    files: &[FileIndexEntry],
    base_output_path: &Path,
    strip_components: Option<u32>,
) -> io::Result<()> {
    // SAFETY: This duplicates the helper from `extract::mod` for now.
    let mut current_offset_in_bundle = 0;
    for file_entry in files {
        let stripped_path = strip_components.map_or_else(
            || file_entry.path.clone(),
            |n| super::strip_path_components(&file_entry.path, n)
        );
        let target_path = base_output_path.join(stripped_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let bytes_to_skip = file_entry.offset_in_bundle - current_offset_in_bundle;
        if bytes_to_skip > 0 {
            io::copy(&mut decoder.take(bytes_to_skip), &mut io::sink())?;
        }

        let stored_sz = if file_entry.stored_size == 0 {
            file_entry.uncompressed_size
        } else {
            file_entry.stored_size
        };

        let mut output_file = File::create(&target_path)?;
        // --- Read preprocessing sentinel + optional meta block ---
        let mut len_buf = [0u8; 4];
        decoder.read_exact(&mut len_buf)?;
        let meta_len = u32::from_le_bytes(len_buf);
        if meta_len != u32::MAX {
            {
            let mut skip_reader = (&mut *decoder).take(meta_len as u64);
            io::copy(&mut skip_reader, &mut io::sink())?;
        }
        }
        // Copy exact uncompressed file bytes
        io::copy(&mut decoder.take(file_entry.uncompressed_size), &mut output_file)?;

        #[cfg(unix)]
        {
            #[cfg(unix)]
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file_entry.permissions {
                crate::fsx::set_unix_permissions(&target_path, mode)?;
            }
        }

        current_offset_in_bundle = file_entry.offset_in_bundle + stored_sz;
    }
    Ok(())
}

/// Extract a single bundle using parallel writes after full in-memory decode.
///
/// Strategy (first iteration):
/// 1. Read the compressed bundle to memory.
/// 2. Decode (if needed) to a single `Vec<u8>`.
/// 3. Spawn a rayon parallel iterator over `files`, each writes its slice to disk.
///    This parallelises heavy filesystem writes which were previously sequential.
///
/// This sacrifices peak RAM (size of one bundle) but is safe for Katana: typical
/// bundle ≤ 1 GiB.  Further optimisations (streamed ring-buffer) can replace this
/// later.
///
/// can later be executed inside a Rayon/worker thread.
#[allow(clippy::too_many_arguments)]
pub fn extract_bundle_sequential(
    archive_path: &Path,
    bundle_info: &crate::archive::BundleInfo,
    files: &[FileIndexEntry],
    index: &ArchiveIndex,
    base_output_path: &Path,
    strip_components: Option<u32>,
) -> io::Result<()> {
    // --- Zero-copy fast path for plain Store bundles ---
    if bundle_info.algo == "store" {
        return extract_store_bundle_zero_copy(archive_path, bundle_info, files, base_output_path, strip_components);
    }
    use std::io::BufReader;

    let mut f = File::open(archive_path)?;
    f.seek(SeekFrom::Start(bundle_info.offset))?;
    let bundle_reader = (&mut f).take(bundle_info.compressed_size);
    let mut buffered_reader = BufReader::new(bundle_reader);

    // Fast-path: if bundle is zstd and large enough, try parallel block decode
    const PARALLEL_THRESHOLD: u64 = 32 * 1024 * 1024; // 32 MiB
    if bundle_info.algo == "zstd" && bundle_info.compressed_size > PARALLEL_THRESHOLD {
        if let Ok(decoded_vec) = block_pipeline::decode_bundle_parallel_blocks(&mut buffered_reader) {
            writer_pool::flush_files(&decoded_vec, files, base_output_path)?;
            return Ok(());
        }
        // On any error fall back to sequential decoder below.
    }

    let mut decoder: Box<dyn Read + Send> = match bundle_info.algo.as_str() {
        "store" => Box::new(buffered_reader),
        "lzma2" => Box::new(xz2::read::XzDecoder::new(buffered_reader)),
        _ => {
            if let Some(dict) = &index.dictionary {
                Box::new(zstd::stream::Decoder::with_dictionary(buffered_reader, dict)?)
            } else {
                Box::new(zstd::stream::Decoder::new(buffered_reader)?)
            }
        }
    };

    extract_from_decoder(&mut decoder, files, base_output_path, strip_components)
}

use rayon::prelude::*;

/// Zero-copy extractor for bundles stored with `CompressionAlgo::Store`.
/// Skips the 4-byte sentinel and copies the remaining bytes directly from the
/// archive file into the destination file using `io::copy`, which on Unix
/// leverages `copy_file_range` for kernel-space transfer.
fn extract_store_bundle_zero_copy(
    archive_path: &Path,
    bundle_info: &crate::archive::BundleInfo,
    files: &[FileIndexEntry],
    base_output_path: &Path,
    strip_components: Option<u32>,
) -> io::Result<()> {
    use std::io::{Read, Seek};
    let mut archive = File::open(archive_path)?;

    for entry in files {
        // Seek to the start of this file inside the bundle.
        let file_offset = bundle_info.offset + entry.offset_in_bundle;
        archive.seek(SeekFrom::Start(file_offset))?;

        // Read 8-byte size prefix (u64 little-endian)
        let mut size_buf = [0u8; 8];
        archive.read_exact(&mut size_buf)?;
        let file_size = u64::from_le_bytes(size_buf);

        // In index, `stored_size` already includes the 8-byte size prefix.
        // We'll copy exactly `file_size` bytes of payload after the prefix.
        let bytes_to_copy = file_size;

        let stripped_path = strip_components.map_or_else(
            || entry.path.clone(),
            |n| super::strip_path_components(&entry.path, n)
        );
        let target_path = base_output_path.join(stripped_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&target_path)?;

        let mut limited_reader = (&mut archive).take(bytes_to_copy);
        io::copy(&mut limited_reader, &mut out)?;

        if let Some(mode) = entry.permissions {
            crate::fsx::set_unix_permissions(&target_path, mode)?;
        }
    }
    Ok(())
}

/// Потоковая параллельная распаковка: каждый поток читает **только** свой диапазон из архива,
/// декодирует и сразу пишет файл, не буферизуя весь бандл.
#[allow(clippy::too_many_arguments)]
pub fn extract_bundle_parallel(
    archive_path: &Path,
    bundle_info: &crate::archive::BundleInfo,
    files: &[FileIndexEntry],
    index: &ArchiveIndex,
    base_output_path: &Path,
    strip_components: Option<u32>,
) -> io::Result<()> {
    // Fast-path: если весь бандл записан в режиме `Store`, параллельная обработка даёт
    // лишь накладные расходы. Используем проверенный последовательный путь.
    if bundle_info.algo == "store" {
        return extract_bundle_sequential(archive_path, bundle_info, files, index, base_output_path, strip_components);
    }

    // Для очень больших бандлов (>2 ГБ) тоже лучше остаться на последовательном пути —
    // объём I/O сопоставим, но избегаем лишних seek'ов.
    if bundle_info.compressed_size > 2 * 1024 * 1024 * 1024 {
        return extract_bundle_sequential(archive_path, bundle_info, files, index, base_output_path, strip_components);
    }

    // 1. Read compressed bundle into memory.
    // Ссылка на архив для замыкания.
    let archive_path_buf = archive_path.to_path_buf();

    // 2. Параллельная обработка файлов.
    files.par_iter().try_for_each(|entry| -> io::Result<()> {
        // Открываем новый дескриптор, чтобы не блокировать другие потоки.
        let mut f = File::open(&archive_path_buf)?;
        let slice_offset = bundle_info.offset + entry.offset_in_bundle;
        f.seek(SeekFrom::Start(slice_offset))?;

        let stored_sz = if entry.stored_size == 0 {
            entry.uncompressed_size
        } else {
            entry.stored_size
        };

        let limited = f.take(stored_sz);
        let buffered = BufReader::new(limited);

        // Подбираем декодер в зависимости от алгоритма бандла.
        let mut decoder: Box<dyn Read> = match bundle_info.algo.as_str() {
            "store" => Box::new(buffered),
            "lzma2" => Box::new(xz2::read::XzDecoder::new(buffered)),
            _ => {
                if let Some(dict) = &index.dictionary {
                    Box::new(zstd::stream::Decoder::with_dictionary(buffered, dict)?)
                } else {
                    Box::new(zstd::stream::Decoder::new(buffered)?)
                }
            }
        };

        // Готовим путь вывода.
        let target_path = base_output_path.join(&entry.path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&target_path)?;

        // Читаем префикс метаданных.
        let mut len_buf = [0u8; 4];
        decoder.read_exact(&mut len_buf)?;
        let meta_len = u32::from_le_bytes(len_buf);
        if meta_len == u32::MAX {
            io::copy(&mut decoder, &mut out)?;
        } else {
            {
            let mut skip_reader = (&mut *decoder).take(meta_len as u64);
            io::copy(&mut skip_reader, &mut io::sink())?;
        }
            io::copy(&mut decoder, &mut out)?;
        }

        if let Some(mode) = entry.permissions {
            crate::fsx::set_unix_permissions(&target_path, mode)?;
        }
        Ok(())
    })?;

    Ok(())
}
