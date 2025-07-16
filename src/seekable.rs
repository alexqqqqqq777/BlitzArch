//! Experimental seekable-archive implementation based on `zstd-seekable`.
//!
//! Format (very simple, round-trip only, not forward-compatible):
//! 1) Payload: pure seekable-zstd stream (concatenated files, no padding).
//! 2) Footer:
//!    [JSON metadata]
//!    [u64 little-endian length of JSON]
//!    [8-byte magic "SKMIDX00"].
//!
//! JSON metadata is `Vec<FileEntry>` with path, size, permissions.
//! This is enough for sequential extraction and random seek via index.

use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;
use zstd_seekable::{SeekableCStream, Seekable};
use zstd_sys::ZSTD_FRAMEHEADERSIZE_MAX;
use serde::{Serialize, Deserialize};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt; // mode()
use memmap2::Mmap;
use zstd::stream::read::Decoder as ZstdDecoder;

/// Footer magic for version 1 (compressed index)
const MAGIC_V1: &[u8; 8] = b"SKMIDX01";
/// Legacy footer magic with raw JSON index (uncompressed)
const MAGIC_V0: &[u8; 8] = b"SKMIDX00";

#[derive(Serialize, Deserialize)]
struct FileEntry {
    path: String,
    size: u64,
    permissions: Option<u32>,
}

/// Creates a seekable archive at `output_path` with the given ZSTD level.
/// All `inputs` (files or directories) are added recursively.
///
/// The resulting file is a pure seekable-zstd stream without any
/// MFA-specific headers – adequate for performance experiments.
pub fn create_seekable_archive_mt(
    inputs: &[PathBuf],
    output_path: &Path,
    level: i32,
    threads: usize,
) -> Result<(), Box<dyn Error>> {
    use rayon::prelude::*;
    if threads <= 1 {
        return create_seekable_archive(inputs, output_path, level);
    }

    // Collect list same as single-thread
    let mut file_list = Vec::new();
    for path in inputs {
        if path.is_file() {
            file_list.push(path.clone());
        } else if path.is_dir() {
            for entry in WalkDir::new(path) {
                let e = entry?;
                if e.file_type().is_file() {
                    file_list.push(e.path().to_path_buf());
                }
            }
        }
    }
    if file_list.is_empty() { return Err("No input files given".into()); }
    println!("[seekable] Compressing {} files with {} threads → {}", file_list.len(), threads, output_path.display());

    // Split files evenly
    let chunk_size = (file_list.len() + threads - 1) / threads;
    let file_chunks: Vec<Vec<PathBuf>> = file_list.chunks(chunk_size).map(|c| c.to_vec()).collect();

    // Parallel compress each chunk
    let results: Vec<(Vec<u8>, Vec<FileEntry>)> = file_chunks
        .into_par_iter()
        .map(|chunk| {
            let mut buffer: Vec<u8> = Vec::with_capacity(4 * 1024 * 1024); // start with 4 MiB, grows as needed
            const KATANA_CHUNK: usize = 8 * 1024 * 1024; // 8 MiB chunk for maximum throughput
            let mut cstream = SeekableCStream::new(level as usize, KATANA_CHUNK).expect("cstream");
            let mut in_buf = vec![0u8; 2 << 20]; // 2 MiB read buffer
            let mut out_buf = vec![0u8; ZSTD_FRAMEHEADERSIZE_MAX as usize * 2];
            let mut local_index = Vec::new();

            for file_path in &chunk {
                let mut f = File::open(file_path).expect("open");
                let meta = f.metadata().expect("meta");
                local_index.push(FileEntry {
                    path: file_path.strip_prefix(Path::new("/")).unwrap().to_string_lossy().into_owned(),
                    size: meta.len(),
                    permissions: Some(meta.permissions().mode()),
                });
                loop {
                    let read_bytes = f.read(&mut in_buf).expect("read");
                    if read_bytes == 0 { break; }
                    let mut consumed = 0;
                    while consumed < read_bytes {
                        let (written, used) = cstream.compress(&mut out_buf, &in_buf[consumed..read_bytes]).expect("compress");
                        buffer.extend_from_slice(&out_buf[..written]);
                        consumed += used;
                    }
                }
            }
            // finish stream
            loop {
                let written = cstream.end_stream(&mut out_buf).expect("end");
                if written == 0 { break; }
                buffer.extend_from_slice(&out_buf[..written]);
            }
            (buffer, local_index)
        })
        .collect();

    // Merge temp files sequentially
    let mut out_file = File::create(output_path)?;
    let mut full_index = Vec::new();
    for (buf, local_index) in results {
        out_file.write_all(&buf)?;
        full_index.extend(local_index);
    }

        // ---- Footer with compressed index ----
    let json = serde_json::to_vec(&full_index)?;
    let compressed = zstd::stream::encode_all(&json[..], 0)?; // fast compression (level 0)
    out_file.write_all(&compressed)?;
    out_file.write_all(&(compressed.len() as u64).to_le_bytes())?; // comp_len
    out_file.write_all(&(json.len() as u64).to_le_bytes())?;      // original json len
    out_file.write_all(MAGIC_V1)?;
    out_file.flush()?;
    println!("[seekable] Archive ready: {} bytes ({} files)", out_file.metadata()?.len(), full_index.len());
    Ok(())
}

/// Creates a seekable archive at `output_path` with the given ZSTD level.
/// All `inputs` (files or directories) are added recursively.
///
/// The resulting file is a pure seekable-zstd stream without any
/// MFA-specific headers – adequate for performance experiments.
pub fn create_seekable_archive(
    inputs: &[PathBuf],
    output_path: &Path,
    level: i32,
) -> Result<(), Box<dyn Error>> {
    // Collect the full file list first – we need sizes for progress if desired.
    let mut file_list = Vec::new();
    for path in inputs {
        if path.is_file() {
            file_list.push(path.clone());
        } else if path.is_dir() {
            for entry in WalkDir::new(path) {
                let e = entry?;
                if e.file_type().is_file() {
                    file_list.push(e.path().to_path_buf());
                }
            }
        }
    }

    if file_list.is_empty() {
        return Err("No input files given".into());
    }
    println!("[seekable] Compressing {} files → {}", file_list.len(), output_path.display());

    let mut out_file = File::create(output_path)?;
    let mut index: Vec<FileEntry> = Vec::new();
    // 256 KB max chunk size (zstd seekable default is 128 KiB). Adjustable later.
    let mut cstream = SeekableCStream::new(level as usize, 128 * 1024)?;

    let mut in_buf = vec![0u8; 2 << 20]; // 2 MiB read buffer // 1 MB read buffer
    let mut out_buf = vec![0u8; ZSTD_FRAMEHEADERSIZE_MAX as usize * 2];

    for file_path in &file_list {
        let mut f = File::open(file_path)?;
        let meta = f.metadata()?;
        index.push(FileEntry {
            path: file_path.strip_prefix(Path::new("/"))?.to_string_lossy().into_owned(),
            size: meta.len(),
            permissions: Some(meta.permissions().mode()),
        });        loop {
            let read_bytes = f.read(&mut in_buf)?;
            if read_bytes == 0 { break; }
            let mut consumed = 0;
            while consumed < read_bytes {
                let (written, used) = cstream.compress(&mut out_buf, &in_buf[consumed..read_bytes])?;
                out_file.write_all(&out_buf[..written])?;
                consumed += used;
            }
        }
    }

    // Finish stream
    loop {
        let written = cstream.end_stream(&mut out_buf)?;
        if written == 0 { break; }
        out_file.write_all(&out_buf[..written])?;
    }
    out_file.flush()?;

        // ---- Footer with compressed index ----
    let json = serde_json::to_vec(&index)?;
    let compressed = zstd::stream::encode_all(&json[..], 0)?;
    out_file.write_all(&compressed)?;
    out_file.write_all(&(compressed.len() as u64).to_le_bytes())?; // comp_len
    out_file.write_all(&(json.len() as u64).to_le_bytes())?;      // json_len
    out_file.write_all(MAGIC_V1)?;
    out_file.flush()?;

    println!("[seekable] Archive ready: {} bytes ({} files)", out_file.metadata()?.len(), index.len());
    Ok(())
}

pub fn extract_seekable_archive(archive: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;
    let mut file = File::open(archive)?;
    let total_size = file.metadata()?.len();
    if total_size < 16 {
        return Err("File too small".into());
    }
    use std::io::{Seek, SeekFrom, Read};
    file.seek(SeekFrom::End(-24))?; // comp_len + json_len + magic
    let mut trailer = [0u8;24];
    file.read_exact(&mut trailer)?;
    let (lens_bytes, magic_bytes) = trailer.split_at(16);
    let (comp_len_bytes, json_len_bytes) = lens_bytes.split_at(8);
    if magic_bytes == MAGIC_V1 {
                let comp_len = u64::from_le_bytes(comp_len_bytes.try_into().unwrap());
        let json_len = u64::from_le_bytes(json_len_bytes.try_into().unwrap());
        if comp_len + 24 > total_size {
            return Err("Corrupted footer".into());
        }
        let index_start = total_size - 24 - comp_len;
        file.seek(SeekFrom::Start(index_start))?;
        let mut comp_buf = vec![0u8; comp_len as usize];
        file.read_exact(&mut comp_buf)?;
        let mut decoder = ZstdDecoder::new(&comp_buf[..])?;
        let mut json = Vec::with_capacity(json_len as usize);
        std::io::copy(&mut decoder, &mut json)?;
        let index: Vec<FileEntry> = serde_json::from_slice(&json)?;

        // Memory-map the payload region (no full copy)
        let mmap = unsafe { Mmap::map(&file)? };
        // For V1 footer, payload ends right before the compressed index.
        let payload_size = (total_size - comp_len - 24) as usize;
        let payload = &mmap[..payload_size];
        let mut seekable = Seekable::init_buf(payload)?;
        let frames = seekable.get_num_frames();

        // Extraction loop
        let mut entry_iter = index.into_iter();
        let mut current = match entry_iter.next() {
            Some(e) => e,
            None => return Ok(()),
        };
        let mut remaining = current.size as usize;
        let mut first_path = output_dir.join(&current.path);
        if let Some(par) = first_path.parent() { fs::create_dir_all(par)?; }
        let mut out = File::create(first_path)?;
        // Single reusable buffer to cap RAM usage during extraction
        let mut buf: Vec<u8> = Vec::with_capacity(128 * 1024);
        for frame_idx in 0..frames {
            let frame_size = seekable.get_frame_decompressed_size(frame_idx);
            if buf.len() < frame_size { buf.resize(frame_size, 0); }
            seekable.decompress_frame(&mut buf[..frame_size], frame_idx);
            let mut pos = 0;
            while pos < frame_size {
                if remaining == 0 {
                    // finish file
                    if let Some(mode) = current.permissions {
                        crate::fsx::set_unix_permissions(&output_dir.join(&current.path), mode)?;
                    }
                    current = match entry_iter.next() { Some(e)=>e, None=> return Ok(()) };
                    out = {
                        let path = output_dir.join(&current.path);
                        if let Some(par)=path.parent(){fs::create_dir_all(par)?;}
                        File::create(path)?
                    };
                    remaining = current.size as usize;
                }
                let chunk = std::cmp::min(remaining, frame_size - pos);
                out.write_all(&buf[pos..pos+chunk])?;
                remaining -= chunk;
                pos += chunk;
            }
        }
        Ok(())
    } else if magic_bytes == MAGIC_V0 {
                // Re-read 16-byte footer (legacy)
        file.seek(SeekFrom::End(-16))?;
        let mut trailer16 = [0u8;16];
        file.read_exact(&mut trailer16)?;
        let (json_len_bytes, _) = trailer16.split_at(8);
        let json_len = u64::from_le_bytes(json_len_bytes.try_into().unwrap());
        if json_len as u64 + 16 > total_size {
            return Err("Corrupted footer".into());
        }
        file.seek(SeekFrom::End(-16 - json_len as i64))?;
        let mut json_buf = vec![0u8; json_len as usize];
        file.read_exact(&mut json_buf)?;
        let index: Vec<FileEntry> = serde_json::from_slice(&json_buf)?;

        // Memory-map the payload region (no full copy)
        let mmap = unsafe { Mmap::map(&file)? };
        let payload_size = (total_size - json_len - 16) as usize;
        let payload = &mmap[..payload_size];
        let mut seekable = Seekable::init_buf(payload)?;
        let frames = seekable.get_num_frames();

        // Extraction loop
        let mut entry_iter = index.into_iter();
        let mut current = match entry_iter.next() {
            Some(e) => e,
            None => return Ok(()),
        };
        let mut remaining = current.size as usize;
        let mut first_path = output_dir.join(&current.path);
        if let Some(par) = first_path.parent() { fs::create_dir_all(par)?; }
        let mut out = File::create(first_path)?;
        // Single reusable buffer to cap RAM usage during extraction
        let mut buf: Vec<u8> = Vec::with_capacity(128 * 1024);
        for frame_idx in 0..frames {
            let frame_size = seekable.get_frame_decompressed_size(frame_idx);
            if buf.len() < frame_size { buf.resize(frame_size, 0); }
            seekable.decompress_frame(&mut buf[..frame_size], frame_idx);
            let mut pos = 0;
            while pos < frame_size {
                if remaining == 0 {
                    // finish file
                    if let Some(mode) = current.permissions {
                        crate::fsx::set_unix_permissions(&output_dir.join(&current.path), mode)?;
                    }
                    current = match entry_iter.next() { Some(e)=>e, None=> return Ok(()) };
                    out = {
                        let path = output_dir.join(&current.path);
                        if let Some(par)=path.parent(){fs::create_dir_all(par)?;}
                        File::create(path)?
                    };
                    remaining = current.size as usize;
                }
                let chunk = std::cmp::min(remaining, frame_size - pos);
                out.write_all(&buf[pos..pos+chunk])?;
                remaining -= chunk;
                pos += chunk;
            }
        }
        Ok(())
    } else {
        return Err("Not a seekable archive (magic)".into());
    }

}
