//! # The Katana Archive Format
//! 
//! This module implements an experimental, high-performance, parallel-friendly archive format named "Katana".
//! 
//! ## Format Specification
//! 
//! The Katana format (`.blz` when used with the `--katana` flag) is designed for maximum creation and extraction speed on modern, multi-core systems. Its structure is as follows:
//! 
//! 1.  **Data Shards**: A sequence of independent, concatenated `zstd` compressed data streams. Each shard is created and can be extracted in parallel.
//! 2.  **JSON Index**: A `zstd`-compressed JSON object containing metadata for all shards and files.
//! 3.  **Footer**: A fixed-size block at the very end of the file containing:
//!     - `index_compressed_size: u64`: The size of the compressed JSON index.
//!     - `index_uncompressed_size: u64`: The original size of the JSON index.
//!     - `magic_bytes: [u8; 8]`: The magic signature `b"KATIDX01"`.
//! 
//! This design allows an extractor to read the footer, locate and decompress the index, and then dispatch multiple threads to decompress the data shards in parallel, achieving very high I/O throughput.

//! Katana: ultra-fast multi-threaded archive writer.
//!
//! The format is deliberately simple:
//! 1. A sequence of independent zstd streams ("shards"), concatenated back-to-back.
//! 2. A zstd-compressed JSON index appended at the end, followed by:
//!        [u64 index_comp_size] [u64 index_json_size] [8-byte magic "KATIDX01"]
//!
//! Each shard knows the list of files it owns, so extraction can run one thread per shard.
//! We do **NOT** use zstd-seekable; instead each shard is one normal zstd stream
//! produced by `zstd::Encoder::new_mt(level 0, nb_threads)`.

use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt; // mode()
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use crate::crypto;

/// Magic footer for Katana index (version 1)
const KATANA_MAGIC: &[u8; 8] = b"KATIDX01";

/// Normalize path by replacing backslashes with forward slashes and maintaining directory structure.
/// Remove unnecessary path components like './' while preserving all directories.
/// Example: "./dir1/dir2/file.txt" becomes "dir1/dir2/file.txt"
pub(crate) fn normalize_path(path: &str) -> String {
    // Заменяем обратные слэши на прямые
    let s = path.replace('\\', "/");
    let trimmed = s.strip_prefix("./").unwrap_or(&s);

    // Сохраняем полную структуру директорий;
    // при необходимости убираем повторные слэши
    let res = trimmed.replace("//", "/");

    // --- Debug log --------------------------------------------------------
    if std::env::var("BLITZ_DEBUG_PATHS").is_ok() {
        eprintln!("[dbg] normalize_path: {} -> {}", path, res);
    }
    // ---------------------------------------------------------------------
    res
}

/// Represents a single file's metadata within the Katana index.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct FileEntry {
    /// The relative path of the file within the archive.
    path: String,
    /// The original, uncompressed size of the file.
    size: u64,
    /// The uncompressed offset of the file within its data shard.
    #[serde(default)]
    offset: u64,
    /// The file's Unix permissions, if available.
    permissions: Option<u32>,
}

/// Represents a single data shard's metadata within the Katana index.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ShardInfo {
    /// The byte offset where this shard's data begins in the archive file.
    offset: u64,
    /// The compressed (or encrypted-compressed) size of the shard's data.
    compressed_size: u64,
    /// The total uncompressed size of all files within this shard.
    uncompressed_size: u64,
    /// The number of files contained within this shard.
    file_count: usize,
    /// 12-byte AES-GCM nonce; `None` ⇒ shard not encrypted.
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<[u8; 12]>,
}

/// The main index structure for a Katana archive.
#[derive(Serialize, Deserialize, Debug)]
struct KatanaIndex {
    /// Optional 16-byte salt used for key derivation when archive is encrypted.
    #[serde(skip_serializing_if = "Option::is_none")]
    salt: Option<[u8; 16]>,
    /// A list of all data shards in the archive.
    shards: Vec<ShardInfo>,
    /// A flat list of all files in the archive, sorted by shard and then by offset.
    files: Vec<FileEntry>,
}

/// Split a list into approx equal chunks
fn split_even<T: Clone>(list: &[T], parts: usize) -> Vec<Vec<T>> {
    let mut chunks = Vec::with_capacity(parts);
    let chunk_size = (list.len() + parts - 1) / parts;
    for c in list.chunks(chunk_size) {
        chunks.push(c.to_vec());
    }
    chunks
}

/// Returns the longest common ancestor directory shared by all provided paths.
/// If the slice is empty, an empty `PathBuf` is returned.
pub(crate) fn common_parent(paths: &[PathBuf]) -> PathBuf {
    use std::path::Component;

    if paths.is_empty() {
        return PathBuf::new();
    }

    // Start with components of the first path
    let mut prefix: Vec<Component> = paths[0].components().collect();
    for p in &paths[1..] {
        let comps: Vec<Component> = p.components().collect();
        let mut idx = 0usize;
        while idx < prefix.len() && idx < comps.len() && prefix[idx] == comps[idx] {
            idx += 1;
        }
        prefix.truncate(idx);
        if prefix.is_empty() {
            break;
        }
    }

    let mut out = PathBuf::new();
    for c in prefix {
        out.push(c.as_os_str());
    }

    // Edge-case: if result is empty and the first path is a file – use its parent.
    if out.as_os_str().is_empty() {
        if let Some(par) = paths[0].parent() {
            return par.to_path_buf();
        }
    }

    out
}

/// Creates a new Katana archive from a set of input files and directories.
///
/// This function orchestrates the parallel compression of files into shards and writes the final archive.
///
/// # Arguments
/// * `inputs` - A slice of paths to files or directories to be archived.
/// * `output_path` - The path where the final `.blz` archive will be created.
/// * `threads` - The number of parallel shards to create. If `0`, it will auto-detect based on the number of CPU cores.
/// * `password` - Optional password for encryption.
pub fn create_katana_archive(
    inputs: &[PathBuf],
    output_path: &Path,
    threads: usize,
    password: Option<String>,
) -> Result<(), Box<dyn Error>> {
    // 1. Enumerate all files
    let mut files = Vec::new();
    for path in inputs {
        if path.is_file() {
            files.push(path.clone());
        } else if path.is_dir() {
            for entry in WalkDir::new(path) {
                let e = entry?;
                if e.file_type().is_file() {
                    files.push(e.path().to_path_buf());
                }
            }
        }
    }
    if files.is_empty() {
        return Err("No input files".into());
    }
    let num_shards = if threads == 0 { num_cpus::get() } else { threads };
    let num_shards = num_shards.max(1);

    println!(
        "[katana] Compressing {} files with {} shards → {}",
        files.len(), num_shards, output_path.display()
    );

    // Determine base directory for relative paths (first input path)
    let base_dir: Arc<PathBuf> = Arc::new(common_parent(inputs));

    // Pre-allocate output file (optional). We'll append as we go.
    let mut out_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(output_path)?;


    // 2. Assign files evenly to shards
    let file_chunks = split_even(&files, num_shards);

    // 3. Each shard compresses its chunk in parallel and writes directly via pwrite
    use crossbeam_channel::bounded;
    // Channel capacity 1 → workers block until coordinator writes, limiting peak RAM
    let (meta_tx, meta_rx) = bounded::<(usize, Vec<u8>, u64, Vec<FileEntry>, Option<[u8; 12]>)>(1);
    #[cfg(unix)]
    let _out_fd = out_file.as_raw_fd();

    // Generate single salt if encryption enabled
    let archive_salt: Option<[u8; 16]> = password.as_ref().map(|_| {
        let v = crypto::generate_salt();
        <[u8; 16]>::try_from(v).unwrap()
    });

    #[cfg(unix)]
use std::os::unix::io::AsRawFd;

    let mut index = KatanaIndex {
        salt: archive_salt,
        shards: Vec::with_capacity(num_shards),
        files: Vec::new(),
    };

    rayon::scope(|s| {
        // Spawn compression workers
        for (shard_id, chunk) in file_chunks.into_iter().enumerate() {
            let meta_tx = meta_tx.clone();
            let base_dir = Arc::clone(&base_dir);
            let password_cl = password.clone();
            let salt_cl = archive_salt;

            s.spawn(move |_| {
                // Calculate total uncompressed size to size zstd encoder buffer (optional)
                let unc_sum: u64 = chunk
                    .iter()
                    .map(|p| p.metadata().map(|m| m.len()).unwrap_or(0))
                    .sum();

                // Prepare zstd encoder
                let zstd_threads = (num_cpus::get() / 2).max(1) as u32;
                // Start with 4 MiB buffer regardless of shard size to avoid large allocations
                let mut encoder = zstd::Encoder::new(Vec::with_capacity(4 * 1024 * 1024), 0)
                    .expect("encoder");
                encoder.include_checksum(true).expect("chk");
                encoder.multithread(zstd_threads).expect("mt");

                let mut local_index = Vec::new();
                let mut uncompressed_written: u64 = 0;

                let mut in_buf = vec![0u8; 2 << 20]; // 2 MiB buffer
                for path in &chunk {
                    let mut f = File::open(path).expect("open");
                    let meta = f.metadata().expect("meta");
                    // Всегда сохраняем полную структуру директорий
                    let rel_path = path
                        .strip_prefix(&*base_dir)
                        .unwrap_or(path)
                        .to_path_buf();
                    let normalized_path = normalize_path(&rel_path.to_string_lossy());
                    local_index.push(FileEntry {
                        path: normalized_path,
                        size: meta.len(),
                        offset: uncompressed_written, // record current offset
                        permissions: crate::fsx::maybe_unix_mode(&meta),
                    });
                    uncompressed_written += meta.len();
                    loop {
                        let rd = f.read(&mut in_buf).expect("read");
                        if rd == 0 {
                            break;
                        }
                        encoder.write_all(&in_buf[..rd]).expect("enc write");
                    }
                }
                let comp_buf = encoder.finish().expect("finish");

                // Critical section: reserve offset and pwrite data


                // Send to coordinator
                let (final_buf, nonce_opt) = if let Some(ref pass) = password_cl {
                        let salt = salt_cl.expect("salt present");
                        let (enc, nonce) = crypto::encrypt(&comp_buf, pass, &salt).expect("encrypt");
                        (enc, Some(nonce))
                    } else {
                        (comp_buf, None)
                    };

                    meta_tx
                        .send((
                            shard_id,
                            final_buf,
                            uncompressed_written,
                            local_index,
                            nonce_opt.map(|n| <[u8;12]>::try_from(n).unwrap()),
                        ))
                    .expect("send meta");
            });
        }

        // Coordinator loop runs inside the same scope, so we can write shards while workers continue
        drop(meta_tx);
        // Temporary buffers to keep deterministic order
        let mut shard_infos: Vec<Option<ShardInfo>> = vec![None; num_shards];
        let mut files_by_shard: Vec<Option<Vec<FileEntry>>> = vec![None; num_shards];

        for (sid, comp_data, unc_size, local_files, nonce_opt) in meta_rx.iter() {
            let offset = out_file.seek(SeekFrom::End(0)).expect("seek");
            out_file.write_all(&comp_data).expect("write shard");

            shard_infos[sid] = Some(ShardInfo {
                offset: offset as u64,
                compressed_size: comp_data.len() as u64,
                uncompressed_size: unc_size,
                file_count: local_files.len(),
                nonce: nonce_opt,
            });
            files_by_shard[sid] = Some(local_files);
        }

        for sid in 0..num_shards {
            if let Some(info) = shard_infos[sid].take() {
                index.shards.push(info);
                if let Some(files) = files_by_shard[sid].take() {
                    index.files.extend(files);
                }
            }
        }
    }); // close rayon::scope

    // 5. Write compressed JSON index + footer
    index.salt = archive_salt;
    // Optional debug print – show first 20 paths before we compress the index
if std::env::var("BLITZ_DEBUG_PATHS").is_ok() {
    let sample: Vec<_> = index.files.iter().take(20).map(|f| f.path.clone()).collect();
    eprintln!("[dbg] index sample ({} paths): {:?}", sample.len(), sample);
}

let index_json = serde_json::to_vec(&index)?;
    let mut encoder = zstd::Encoder::new(Vec::new(), 3)?;
    encoder.write_all(&index_json)?;
    let index_comp = encoder.finish()?;

    let index_comp_size = index_comp.len() as u64;
    let index_json_size = index_json.len() as u64;

    out_file.write_all(&index_comp)?;

    out_file.write_all(&index_comp_size.to_le_bytes())?;
    out_file.write_all(&index_json_size.to_le_bytes())?;
    out_file.write_all(KATANA_MAGIC)?;

    println!(
        "[katana] Finished archive: {} shards, {:.2} MiB compressed index",
        index.shards.len(),
        index_comp_size as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}

/// Checks if a file is a valid Katana archive by reading its footer magic bytes.
///
/// This provides a quick and efficient way to identify Katana archives without parsing the full structure.
pub fn is_katana_archive(path: &Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    if len < 8 {
        return Ok(false);
    }
    f.seek(SeekFrom::End(-8))?;
    let mut magic = [0u8; 8];
    f.read_exact(&mut magic)?;
    Ok(&magic == KATANA_MAGIC)
}

/// Extracts a Katana archive to a specified output directory.
///
/// This function will extract the entire contents of the archive.
///
/// # Arguments
/// * `archive_path` - The path to the Katana archive file.
/// * `output_dir` - The directory where the contents will be extracted.
pub fn extract_katana_archive(
    archive_path: &Path,
    output_dir: &Path,
    password: Option<String>,
) -> Result<(), Box<dyn Error>> {
    extract_katana_archive_internal(archive_path, output_dir, &[], password)
}

/// Lists all files in a Katana archive without extracting them.
///
/// This function reads the index of a Katana archive and prints the list of contained files.
///
/// # Arguments
/// * `archive_path` - The path to the Katana archive file.
/// * `password` - Optional password for encrypted archives.
pub fn list_katana_files(
    archive_path: &Path,
    password: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let mut f = File::open(archive_path)?;
    let len = f.metadata()?.len();
    if len < 24 {
        return Err("File too small".into());
    }
    // Read footer
    f.seek(SeekFrom::End(-24))?;
    let mut buf_footer = [0u8; 24];
    f.read_exact(&mut buf_footer)?;
    let (idx_comp_size_bytes, rest) = buf_footer.split_at(8);
    let (idx_json_size_bytes, magic_bytes) = rest.split_at(8);
    if magic_bytes != KATANA_MAGIC {
        return Err("Not a Katana archive".into());
    }
    let idx_comp_size = u64::from_le_bytes(idx_comp_size_bytes.try_into().unwrap());
    let _idx_json_size = u64::from_le_bytes(idx_json_size_bytes.try_into().unwrap());

    // Read compressed index
    let idx_comp_offset = len - 24 - idx_comp_size;
    f.seek(SeekFrom::Start(idx_comp_offset))?;
    let mut idx_comp = vec![0u8; idx_comp_size as usize];
    f.read_exact(&mut idx_comp)?;
    let idx_json = zstd::decode_all(&*idx_comp)?;
    let index: KatanaIndex = serde_json::from_slice(&idx_json)?;
    
    // Print archive information
    if index.salt.is_some() && password.is_none() {
        println!("Archive is encrypted.");
    }
    
    println!("Archive Index ({} files):", index.files.len());
    
    // Print the list of files
    for file in &index.files {
        println!("- {} ({} bytes)", file.path, file.size);
    }
    
    Ok(())
}

/// Internal helper that accepts a list of files to extract. Empty slice ⇒ extract all.
pub fn extract_katana_archive_internal(
    archive_path: &Path,
    output_dir: &Path,
    selected_files: &[PathBuf],
    password: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let mut f = File::open(archive_path)?;
    let len = f.metadata()?.len();
    if len < 24 {
        return Err("File too small".into());
    }
    // Read footer
    f.seek(SeekFrom::End(-24))?;
    let mut buf_footer = [0u8; 24];
    f.read_exact(&mut buf_footer)?;
    let (idx_comp_size_bytes, rest) = buf_footer.split_at(8);
    let (idx_json_size_bytes, magic_bytes) = rest.split_at(8);
    if magic_bytes != KATANA_MAGIC {
        return Err("Not a Katana archive".into());
    }
    let idx_comp_size = u64::from_le_bytes(idx_comp_size_bytes.try_into().unwrap());
    let _idx_json_size = u64::from_le_bytes(idx_json_size_bytes.try_into().unwrap());

    // Read compressed index
    let idx_comp_offset = len - 24 - idx_comp_size;
    f.seek(SeekFrom::Start(idx_comp_offset))?;
    let mut idx_comp = vec![0u8; idx_comp_size as usize];
    f.read_exact(&mut idx_comp)?;
    let idx_json = zstd::decode_all(&*idx_comp)?;
    let index: KatanaIndex = serde_json::from_slice(&idx_json)?;

    // Prepare shard file slices
    let mut file_cursor = 0usize;
    let shards = index.shards.clone();
    let shard_count = shards.len();
    // Precompute stats for final summary
    let total_comp: u64 = shards.iter().map(|s| s.compressed_size).sum();
    let total_uncomp: u64 = shards.iter().map(|s| s.uncompressed_size).sum();
    let ratio = if total_comp > 0 {
        total_uncomp as f64 / total_comp as f64
    } else { 0.0 };
    let files_all = index.files;
    use std::collections::{HashSet};
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    let wanted: HashSet<String> = selected_files
        .iter()
        .map(|p| {
            normalize_path(&p.to_string_lossy())
        })
        .collect();

    let had_error = Arc::new(AtomicBool::new(false));

    let salt_opt = index.salt;
    println!(
        "[katana] Extracting {} shards (filter: {} files)…",
        shards.len(),
        wanted.len()
    );
    rayon::scope(|s| {
        for shard_info in shards.iter().cloned() {
            let archive_path = archive_path.to_path_buf();
            let out_root = output_dir.to_path_buf();
            let shard_files_slice = &files_all[file_cursor..file_cursor + shard_info.file_count];
            file_cursor += shard_info.file_count;

            let need_shard = wanted.is_empty() || shard_files_slice.iter().any(|f| wanted.contains(&f.path));
            if !need_shard {
                continue; // skip shard entirely
            }

            let password_cl = password.clone();
            let salt_cl = salt_opt;
            let error_flag = had_error.clone();
            let wanted_cl = wanted.clone();
            // Pass full slice to maintain correct byte positions
            let shard_vec: Vec<FileEntry> = shard_files_slice.to_vec();
            s.spawn(move |_| {
                if let Err(e) = extract_katana_shard(
                    &archive_path,
                    &out_root,
                    &shard_info,
                    &shard_vec,
                    &wanted_cl,
                    salt_cl,
                    password_cl.as_deref(),
                ) {
                    eprintln!("[katana] shard extract error: {}", e);
                    error_flag.store(true, Ordering::SeqCst);
                }
            });
        }
    });
    if had_error.load(Ordering::SeqCst) {
        return Err("One or more shards failed".into());
    }
    println!(
        "[katana] ✅ Extract complete | Files: {} | Shards: {} | Size: {:.2} → {:.2} MiB (ratio {:.2}x) | CRC: all ok",
        files_all.len(),
        shard_count,
        total_uncomp as f64 / (1024.0 * 1024.0),
        total_comp as f64 / (1024.0 * 1024.0),
        ratio,
    );
    Ok(())
}

use std::collections::HashSet;

fn extract_katana_shard(
    archive_path: &Path,
    out_root: &Path,
    shard_info: &ShardInfo,
    files: &[FileEntry],
    wanted: &HashSet<String>,
    archive_salt: Option<[u8; 16]>,
    password: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    use std::io::{BufWriter, Cursor, Read};
    let mut shard_file = File::open(archive_path)?;
    shard_file.seek(SeekFrom::Start(shard_info.offset))?;

    // Build a reader depending on encryption
    let reader: Box<dyn Read> = if let Some(nc) = shard_info.nonce {
        // --- Encrypted shard: we still need the whole ciphertext in memory ---
        let mut comp_buf = Vec::with_capacity(shard_info.compressed_size as usize);
        std::io::Read::by_ref(&mut shard_file)
            .take(shard_info.compressed_size)
            .read_to_end(&mut comp_buf)?;
        let salt = archive_salt.ok_or("Missing salt in encrypted archive")?;
        let pass = password.ok_or("Password required for encrypted archive")?;
        let dec = crate::crypto::decrypt(&comp_buf, pass, &salt, &nc)
            .map_err(|e| format!("decrypt failed: {:?}", e))?;
        Box::new(Cursor::new(dec))
    } else {
        // --- Not encrypted: stream directly from file, no large allocation ---
        shard_file.seek(SeekFrom::Start(shard_info.offset))?;
        Box::new(shard_file.take(shard_info.compressed_size))
    };

    let mut decoder = zstd::stream::read::Decoder::new(reader)?;

    let mut in_buf = [0u8; 1 << 16];
    for entry in files {
        let mut remaining = entry.size;
        if wanted.is_empty() || wanted.contains(&entry.path) {
            // Write this file to disk
            let out_path = out_root.join(&entry.path);
            if let Some(dir) = out_path.parent() {
                fs::create_dir_all(dir)?;
            }
            if std::env::var("BLITZ_DEBUG_PATHS").is_ok() {
                    eprintln!("[dbg] extract -> {:?}", out_path);
                }
                let target_path = out_path.clone();
            let mut out_f = BufWriter::new(File::create(&out_path)?);
            while remaining > 0 {
                let to_read = std::cmp::min(in_buf.len() as u64, remaining) as usize;
                let rd = decoder.read(&mut in_buf[..to_read])?;
                if rd == 0 {
                    return Err("Unexpected EOF while decoding shard".into());
                }
                out_f.write_all(&in_buf[..rd])?;
                remaining -= rd as u64;
            }
            out_f.flush()?;
            if let Some(perm) = entry.permissions {
                crate::fsx::set_unix_permissions(&out_path, perm)?;
            }
        } else {
            // Skip this file's bytes
            while remaining > 0 {
                let to_read = std::cmp::min(in_buf.len() as u64, remaining) as usize;
                let rd = decoder.read(&mut in_buf[..to_read])?;
                if rd == 0 {
                    return Err("Unexpected EOF while skipping".into());
                }
                remaining -= rd as u64;
            }
        }
    }
    Ok(())
}
