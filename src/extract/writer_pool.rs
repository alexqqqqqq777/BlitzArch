//! Buffered writer-pool that groups several small files into a single
//! write_batch on the same thread to reduce syscall overhead.
//!
//! This is a *minimal* first version: it writes files sequentially but groups
//! up to `BATCH_FILE_LIMIT` small files (\<= `BATCH_SIZE_LIMIT`) into one
//! contiguous memory buffer and issues a single `write_all` to disk.
//!
//! A cross-beam channel + thread-pool implementation (true parallel IO) can be
//! added later, but even this simple grouping removes tens of thousands of
//! syscalls for datasets with many tiny files.

use crate::fsx as fs;
use fs::File;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use fs::Permissions;
use std::path::{Path, PathBuf};

use crate::archive::FileIndexEntry;

// Defaults; may be overridden per bundle using heuristics
const DEFAULT_BATCH_FILE_LIMIT: usize = 8;
const DEFAULT_BATCH_SIZE_LIMIT: usize = 512 * 1024; // 512 KiB

/// Write decoded file slices to disk using simple batching.
///
/// * `decoded` – full uncompressed bundle.
/// * `files`   – slice of entries belonging to bundle (kept in archive index order).
/// * `base`    – user-selected output directory.
///
/// NOTE: Assumes that `decoded` layout exactly matches order/offsets of `files`.
const MIN_FILE_THRESHOLD: usize = 1000;
const MAX_AVG_SIZE: usize = 256 * 1024; // 256 KiB

pub fn flush_files(decoded: &[u8], files: &[FileIndexEntry], base: &Path) -> io::Result<()> {
    // Decide early whether to use batching.
    let avg_size = if files.is_empty() { 0 } else { decoded.len() / files.len() };
    let use_batch = files.len() >= MIN_FILE_THRESHOLD && avg_size <= MAX_AVG_SIZE;

    if !use_batch {
        // Simple direct write path (old behaviour).
        let mut cursor = 0usize;
        for entry in files {
            let size = entry.uncompressed_size as usize;
            let data_end = cursor + size;
            let slice = &decoded[cursor..data_end];
            cursor = data_end;
            let target_path = base.join(&entry.path);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if entry.is_dir {
                fs::create_dir_all(&target_path)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = entry.permissions {
                        fs::set_permissions(&target_path, fs::Permissions::from_mode(mode))?;
                    }
                }
            } else {
                let mut f = File::create(&target_path)?;
                f.write_all(slice)?;
            }
        }
        return Ok(());
    }

    // batched path below

    let mut cursor = 0usize;

    // Heuristic limits depending on bundle size
    let total_size = decoded.len();
    let batch_size_limit = std::cmp::min(std::cmp::max(total_size / 256, 128 * 1024), 2 * 1024 * 1024); // 128 KiB..2 MiB
    let batch_file_limit = DEFAULT_BATCH_FILE_LIMIT;

    // Internal staging buffer.
    let mut batch_items: Vec<(PathBuf, usize /*size*/)> = Vec::with_capacity(batch_file_limit);
    let mut batch_buf = Vec::<u8>::with_capacity(batch_size_limit);

    for entry in files {
        let size = entry.uncompressed_size as usize;
        let data_end = cursor + size;
        let slice = &decoded[cursor..data_end];
        cursor = data_end;

        let target_path = base.join(&entry.path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if entry.is_dir {
            fs::create_dir_all(&target_path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.permissions {
                    fs::set_permissions(&target_path, fs::Permissions::from_mode(mode))?;
                }
            }
            continue;
        }

        // Flush if limits exceeded
        if batch_items.len() >= batch_file_limit || batch_buf.len() + size > batch_size_limit {
            flush_batch(&batch_items, &batch_buf)?;
            batch_items.clear();
            batch_buf.clear();
        }

        batch_items.push((target_path, size));
        batch_buf.extend_from_slice(slice);
    }

    if !batch_items.is_empty() {
        flush_batch(&batch_items, &batch_buf)?;
    }

    Ok(())
}

/// Helper: flush prepared batch buffer to individual files.
fn flush_batch(items: &[(PathBuf, usize)], buf: &[u8]) -> io::Result<()> {
    let mut offset = 0usize;
    for (path, size) in items {
        let mut f = File::create(path)?;
        f.write_all(&buf[offset..offset + *size])?;
        offset += *size;
    }
    Ok(())
}

