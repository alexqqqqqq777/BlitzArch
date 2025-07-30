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
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write, BufWriter};
use scopeguard;

// ---------- Footer constants (added for compatibility with new BLAKE3 footer) --------
/// 16-byte magic that marks optional integrity footer written by `katana_stream`.
const FOOTER_MAGIC: &[u8; 16] = b"KATANA_HASH_FOOT";
const FOOTER_SIZE: u64 = 16 + 8 + 32; // magic + data_len (u64) + blake3 (32)

/// If the file ends with the optional BLAKE3 footer, returns `file_len - FOOTER_SIZE`,
/// otherwise returns original `file_len`.
fn data_len_without_footer(f: &mut File, file_len: u64) -> std::io::Result<u64> {
    use std::io::{Read, Seek};
    if file_len >= FOOTER_SIZE {
        // Peek last 16 bytes and compare magic
        f.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))?;
        let mut magic_buf = [0u8; 16];
        f.read_exact(&mut magic_buf)?;
        if &magic_buf == FOOTER_MAGIC {
            // Next 8 bytes – original data length
            let mut len_bytes = [0u8; 8];
            f.read_exact(&mut len_bytes)?;
            let data_len = u64::from_le_bytes(len_bytes);
            return Ok(data_len);
        }
    }
    Ok(file_len)
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt; // mode()
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use crate::crypto;
use crate::progress::{ProgressTracker, ProgressState};

/// Decrypts AES-GCM ciphertext provided as a reader (ciphertext body) and writes plaintext to writer.
/// `tag` must be the 16-byte authentication tag located at the end of the ciphertext stream.
use aes_gcm_stream::Aes256GcmStreamDecryptor;

fn decrypt_stream_prekey<R: Read, W: Write>(mut rdr: R, mut wtr: W, key: &[u8; 32], nonce: &[u8; 12], tag: &[u8; 16]) -> Result<(), Box<dyn Error>> {
    use std::io::Read;
    // Read the ciphertext body into memory in chunks (few MiB) to avoid many allocations.
    // Stream decrypt to avoid allocating whole shard.
    const CHUNK: usize = 256 * 1024; // 256 KiB
    let mut decryptor = Aes256GcmStreamDecryptor::new(*key, nonce);
    let mut buf = [0u8; CHUNK];
    loop {
        let n = rdr.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let out = decryptor.update(&buf[..n]);
        if !out.is_empty() {
            wtr.write_all(&out)?;
        }
    }
    // feed tag bytes (may output last partial plaintext)
    let tag_out = decryptor.update(tag);
    if !tag_out.is_empty() {
        wtr.write_all(&tag_out)?;
    }
    let final_block = decryptor.finalize().map_err(|e| format!("decrypt failed: {}", e))?;
    if !final_block.is_empty() {
        wtr.write_all(&final_block)?;
    }
    Ok(())
}

/// Magic footer for Katana index (version 1)
const KATANA_MAGIC: &[u8; 8] = b"KATIDX01";

/// Normalize path by replacing backslashes with forward slashes and maintaining directory structure.
/// Remove unnecessary path components like './' while preserving all directories.
/// Example: "./dir1/dir2/file.txt" becomes "dir1/dir2/file.txt"
pub(crate) fn normalize_path(path: &str) -> String {
    // 1. Упрощённая нормализация: unify separators and remove leading "./"
    let s = path.replace('\\', "/");
    let trimmed = s.strip_prefix("./").unwrap_or(&s);
    let collapsed = trimmed.replace("//", "/");

    // 2. Дополнительная санитация только для Windows
    #[cfg(windows)]
    let sanitized = {
        // Обрабатываем каждый компонент пути отдельно, чтобы не затронуть разделители
        let components: Vec<String> = collapsed
            .split('/')
            .filter(|c| !c.is_empty())
            .map(|comp| sanitize_windows_component(comp))
            .collect();
        if components.is_empty() {
            "_".to_string()
        } else {
            components.join("/")
        }
    };

    #[cfg(not(windows))]
    let sanitized = collapsed;

    // --- Debug log --------------------------------------------------------
    if std::env::var("BLITZ_DEBUG_PATHS").is_ok() {
        eprintln!("[dbg] normalize_path: {} -> {}", path, sanitized);
    }
    // ---------------------------------------------------------------------
    sanitized
}

#[cfg(windows)]
fn sanitize_windows_component(name: &str) -> String {

    // 2.1 Удаляем недопустимые символы
    let mut out: String = name
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '\\' | '/' | '|' | '?' | '*' => '_',
            c if (c as u32) < 32 => '_' , // управляющие символы 0-31
            c => c,
        })
        .collect();

    // 2.2 Удаляем завершающие пробелы и точки
    while out.ends_with(' ') || out.ends_with('.') {
        out.pop();
    }

    if out.is_empty() {
        out.push('_');
    }

    // 2.3 Проверяем зарезервированные имена (без учёта регистра и расширения)
    let upper_no_ext = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    if is_windows_reserved(&upper_no_ext) {
        out.push('_');
    }

    out
}

#[cfg(windows)]
fn is_windows_reserved(name_upper: &str) -> bool {
    match name_upper {
        "CON" | "PRN" | "AUX" | "NUL" |
        "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9" |
        "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9" => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_path;

    #[test]
    fn test_normalize_simple() {
        assert_eq!(normalize_path("./dir1/dir2/file.txt"), "dir1/dir2/file.txt");
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_sanitization() {
        // Недопустимые символы заменяются, пробелы/точки убираются, зарезервированные имена модифицируются
        assert_eq!(normalize_path("CON \\foo\\bar?.txt"), "CON_/foo/bar_.txt");
    }
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
    crc32: u32,
    /// 12-byte AES-GCM nonce; `None` ⇒ shard not encrypted.
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<[u8; 12]>,
}

/// The main index structure for a Katana archive.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct KatanaIndex {
    /// CRC32 of the JSON representation for integrity (always present)
    #[serde(default)]
    crc32: u32,
    /// Optional HMAC-SHA256 of JSON when archive is encrypted (Argon2 derived key)
    #[serde(skip_serializing_if = "Option::is_none")]
    hmac: Option<[u8; 32]>,

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
    // Call new katana_stream implementation with default parameters
    crate::katana_stream::create_katana_archive(
        inputs, 
        output_path, 
        threads, 
        0, // codec_threads - auto
        None, // mem_budget_mb - auto
        password, 
        None, // compression_level - auto
        None::<fn(crate::progress::ProgressState)>, // no progress callback
    )
}

/// Creates a new Katana archive with optional progress tracking.
///
/// This is the internal implementation that supports progress callbacks.
///
/// # Arguments
/// * `inputs` - A slice of paths to files or directories to be archived.
/// * `output_path` - The path where the final `.blz` archive will be created.
/// * `threads` - The number of parallel shards to create. If `0`, it will auto-detect based on the number of CPU cores.
/// * `password` - Optional password for encryption.
/// * `progress_callback` - Optional callback for progress updates.
pub fn create_katana_archive_with_progress<F>(
    inputs: &[PathBuf],
    output_path: &Path,
    threads: usize,
    codec_threads: u32,
    mem_budget_mb: Option<u64>,
    password: Option<String>,
    progress_callback: Option<F>,
) -> Result<(), Box<dyn Error>>
where
    F: Fn(ProgressState) + Send + Sync + 'static,
{
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

    // ── Определяем количество потоков кодека в зависимости от budget/параметра ──
    let codec_thr_auto: u32 = if codec_threads > 0 {
        codec_threads
    } else {
        if let Some(mb) = mem_budget_mb {
            if mb > 0 {
                // 4 MiB flush buffer * 3 in-flight = ~12 MiB per thread
                let bytes_per_thread: u64 = 4 * 1024 * 1024 * 3;
                let budget_bytes = mb * 1024 * 1024;
                let est = std::cmp::max(1, (budget_bytes / bytes_per_thread) as u32);
                std::cmp::min(est, num_cpus::get() as u32)
            } else {
                num_cpus::get() as u32
            }
        } else {
            num_cpus::get() as u32
        }
    };

    
    // Calculate total size for progress tracking
    let total_bytes: u64 = files.iter()
        .map(|p| p.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();
    
    // Initialize progress tracker
    let mut progress_tracker = ProgressTracker::new(num_shards, std::time::Duration::from_millis(50));
    if let Some(callback) = progress_callback {
        progress_tracker.enable_with_callback(callback);
        progress_tracker.set_totals(files.len() as u64, total_bytes, num_shards);
    }
    let progress_tracker = std::sync::Arc::new(std::sync::Mutex::new(progress_tracker));

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

    // Pre-derive encryption key once (memory safe)
    use std::sync::Arc;
    let key_bytes_arc: Option<Arc<[u8; 32]>> = if let (Some(pass), Some(ref salt)) = (password.as_ref(), archive_salt.as_ref()) {
        Some(Arc::new(crypto::derive_key_argon2(pass, &salt[..])))
    } else { None };

    let mut index = KatanaIndex {
        crc32: 0,
        hmac: None,
        salt: archive_salt,
        shards: Vec::with_capacity(num_shards),
        files: Vec::new(),
    };

    rayon::scope(|s| {
        // Spawn compression workers
        for (shard_id, chunk) in file_chunks.into_iter().enumerate() {
            let meta_tx = meta_tx.clone();
            let base_dir = Arc::clone(&base_dir);
            
            let key_arc_cl = key_bytes_arc.clone();
            let progress_tracker_cl = Arc::clone(&progress_tracker);
            
            // Get thread-specific metrics handle
            let thread_metrics = {
                let tracker = progress_tracker_cl.lock().unwrap();
                tracker.get_thread_metrics(shard_id)
            };

            s.spawn(move |_| {
                // Calculate total uncompressed size to size zstd encoder buffer (optional)
                let unc_sum: u64 = chunk
                    .iter()
                    .map(|p| p.metadata().map(|m| m.len()).unwrap_or(0))
                    .sum();

                // Prepare zstd encoder
                let zstd_threads = codec_thr_auto.max(1);
                // Start with 4 MiB buffer regardless of shard size to avoid large allocations
                let mut encoder = zstd::Encoder::new(Vec::with_capacity(4 * 1024 * 1024), 0)
                    .expect("encoder");
                encoder.include_checksum(true).expect("chk");
                encoder.multithread(zstd_threads).expect("mt");

                let mut local_index = Vec::new();
                let mut uncompressed_written: u64 = 0;

                let mut in_buf = vec![0u8; 4 * 1024 * 1024]; // Keep 4 MiB for compatibility - will optimize later
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
                    
                    // Record file processed (zero-overhead when progress disabled)
                    if let Some(ref metrics) = thread_metrics {
                        metrics.record_file_processed(meta.len());
                    }
                }
                let comp_buf = encoder.finish().expect("finish");

                // Critical section: reserve offset and pwrite data


                // Send to coordinator
                let (final_buf, nonce_opt) = if let Some(ref key_bytes) = key_arc_cl {
                        let mut comp_buf = comp_buf; // take ownership
let nonce_vec = crypto::encrypt_prekey_in_place(&mut comp_buf, key_bytes).expect("encrypt");
let enc = comp_buf;
                        (enc, Some(nonce_vec))
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

            let shard_crc = crc32fast::hash(&comp_data);
            shard_infos[sid] = Some(ShardInfo {
                offset: offset as u64,
                compressed_size: comp_data.len() as u64,
                uncompressed_size: unc_size,
                file_count: local_files.len(),
                crc32: shard_crc,
                nonce: nonce_opt,
            });
            files_by_shard[sid] = Some(local_files);
            
            // Record shard completion and emit progress
            {
                let tracker = progress_tracker.lock().unwrap();
                tracker.record_shard_completed();
            }
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

// --- Integrity codes -------------------------------------------------------
    use crc32fast::Hasher as Crc32Hasher;
    let mut hasher = Crc32Hasher::new();
    let tmp_json = serde_json::to_vec(&index)?;
    hasher.update(&tmp_json);
    let crc32_val = hasher.finalize();

    let mut index_with_hash = index.clone();
    index_with_hash.crc32 = crc32_val;

    // If encrypted, compute HMAC-SHA256 with key derived from password+salt
    if let (Some(pass), Some(salt)) = (password, archive_salt) {
        use hmac::{Hmac, Mac};
        use sha2::Sha256 as Sha256Mac;
        type HmacSha256 = Hmac<Sha256Mac>;
        let key = crypto::derive_key_argon2(&pass, &salt);
        let mut mac = HmacSha256::new_from_slice(&key).expect("HMAC new");
        mac.update(&tmp_json);
        let result = mac.finalize();
        let bytes = result.into_bytes();
        let mut h = [0u8; 32];
        h.copy_from_slice(&bytes);
        index_with_hash.hmac = Some(h);
    }

    let index_json = serde_json::to_vec(&index_with_hash)?;
    let mut encoder = zstd::Encoder::new(Vec::new(), 3)?;
    encoder.write_all(&index_json)?;
    let index_comp = encoder.finish()?;

    let index_comp_size = index_comp.len() as u64;
    let index_json_size = index_json.len() as u64;

    out_file.write_all(&index_comp)?;

    out_file.write_all(&index_comp_size.to_le_bytes())?;
    out_file.write_all(&index_json_size.to_le_bytes())?;
    out_file.write_all(KATANA_MAGIC)?;

    // Final progress update and statistics
    {
        let tracker = progress_tracker.lock().unwrap();
        let final_state = tracker.get_progress_state();
        
        // Итоговый размер архива со всеми шард-данными
        let compressed_size = out_file.metadata().map(|m| m.len()).unwrap_or(0);

        println!(
            "[katana] Archive complete | Files: {} | Time: {:.1}s | Ratio: {:.2}:1 | Speed: {:.1} MB/s",
            final_state.processed_files,
            final_state.elapsed_time.as_secs_f32(),
            if compressed_size > 0 { total_bytes as f64 / compressed_size as f64 } else { 0.0 },
            final_state.speed_mbps
        );
        
        // Force final progress emission to 100%
        tracker.force_completion();
    }

    Ok(())
}

/// Checks if a file is a valid Katana archive by reading its footer magic bytes.
///
/// This provides a quick and efficient way to identify Katana archives without parsing the full structure.
pub fn is_katana_archive(path: &Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let file_len = f.metadata()?.len();
    let data_len = data_len_without_footer(&mut f, file_len)?;
    if data_len < 8 {
        return Ok(false);
    }
    f.seek(SeekFrom::Start(data_len - 8))?;
    let mut magic = [0u8; 8];
    f.read_exact(&mut magic)?;
    Ok(&magic == KATANA_MAGIC)
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
    let file_len = f.metadata()?.len();
    let data_len = data_len_without_footer(&mut f, file_len)?;
    if data_len < 24 {
        return Err("File too small".into());
    }
    // Read footer
    f.seek(SeekFrom::Start(data_len - 24))?;
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
    let idx_comp_offset = data_len - 24 - idx_comp_size;
    f.seek(SeekFrom::Start(idx_comp_offset))?;
    let mut idx_comp = vec![0u8; idx_comp_size as usize];
    f.read_exact(&mut idx_comp)?;
    let idx_json = zstd::decode_all(&*idx_comp)?;
    let index: KatanaIndex = serde_json::from_slice(&idx_json)?;
    // ---------------- Integrity verification ------------------
    use crc32fast::Hasher as Crc32Hasher;
    // При создании архива вычисляется CRC по JSON с нулевым полем crc32.
    // Для корректной проверки воспроизводим тот же алгоритм.
    let mut index_for_crc = index.clone();
    index_for_crc.crc32 = 0;
    index_for_crc.hmac = None; // CRC вычисляется по JSON без HMAC
    let idx_json_zero = serde_json::to_vec(&index_for_crc)?;
    let mut hasher = Crc32Hasher::new();
    hasher.update(&idx_json_zero);
    let crc_now = hasher.finalize();
    if index.crc32 != 0 && index.crc32 != crc_now {
        return Err("Index CRC mismatch".into());
    }
    if let Some(expected_hmac) = &index.hmac {
        if let (Some(pass), Some(salt)) = (password.as_ref(), index.salt) {
            use hmac::{Hmac, Mac};
            use sha2::Sha256 as Sha256Mac;
            type HmacSha256 = Hmac<Sha256Mac>;
            let key = crypto::derive_key_argon2(&pass, &salt);
            // Для проверки HMAC нужно сериализовать индекс с hmac = None,
            // ровно так же, как при вычислении в create_katana_archive.
            let mut idx_no_hmac = index.clone();
            idx_no_hmac.crc32 = 0;
            idx_no_hmac.hmac = None;
            let idx_json_no_hmac = serde_json::to_vec(&idx_no_hmac)?;
            let mut mac = HmacSha256::new_from_slice(&key).expect("HMAC new");
            mac.update(&idx_json_no_hmac);
            if mac.verify_slice(expected_hmac).is_err() {
                return Err("Index HMAC verification failed".into());
            }
        } else {
            return Err("Encrypted archive: password required for HMAC verification".into());
        }
    }
    
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
    strip_components: Option<u32>,
) -> Result<(), Box<dyn Error>> {
    extract_katana_archive_with_progress(archive_path, output_dir, selected_files, password, strip_components, None::<fn(ProgressState)>)
}

/// Public wrapper for Katana extraction with optional real-time progress.
///
/// This thin wrapper forwards to `extract_katana_archive_with_progress_impl` so that
/// callers (CLI, GUI, Tauri) can link against a stable API while implementation
/// details remain private.
pub fn extract_katana_archive_with_progress<F>(
    archive_path: &Path,
    output_dir: &Path,
    selected_files: &[PathBuf],
    password: Option<String>,
    strip_components: Option<u32>,
    progress_callback: Option<F>,
) -> Result<(), Box<dyn Error>>
where
    F: Fn(ProgressState) + Send + Sync + 'static,
{
    extract_katana_archive_with_progress_impl(
        archive_path,
        output_dir,
        selected_files,
        password,
        strip_components,
        progress_callback,
    )
}

/// Internal implementation of Katana extraction with progress support.
fn extract_katana_archive_with_progress_impl<F>(
    archive_path: &Path,
    output_dir: &Path,
    selected_files: &[PathBuf],
    password: Option<String>,
    strip_components: Option<u32>,
    progress_callback: Option<F>,
) -> Result<(), Box<dyn Error>>
where
    F: Fn(ProgressState) + Send + Sync + 'static,
{
    let mut f = File::open(archive_path)?;
    let file_len = f.metadata()?.len();
    let data_len = data_len_without_footer(&mut f, file_len)?;
    if data_len < 24 {
        return Err("File too small".into());
    }
    // Read footer
    f.seek(SeekFrom::Start(data_len - 24))?;
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
    let idx_comp_offset = data_len - 24 - idx_comp_size;
    f.seek(SeekFrom::Start(idx_comp_offset))?;
    let mut idx_comp = vec![0u8; idx_comp_size as usize];
    f.read_exact(&mut idx_comp)?;
    let idx_json = zstd::decode_all(&*idx_comp)?;
    let index: KatanaIndex = serde_json::from_slice(&idx_json)?;
    // ---------------- Integrity verification ------------------
    use crc32fast::Hasher as Crc32Hasher;
    // При создании архива вычисляется CRC по JSON с нулевым полем crc32.
    // Для корректной проверки воспроизводим тот же алгоритм.
    let mut index_for_crc = index.clone();
    index_for_crc.crc32 = 0;
    index_for_crc.hmac = None; // CRC вычисляется по JSON без HMAC
    let idx_json_zero = serde_json::to_vec(&index_for_crc)?;
    let mut hasher = Crc32Hasher::new();
    hasher.update(&idx_json_zero);
    let crc_now = hasher.finalize();
    if index.crc32 != 0 && index.crc32 != crc_now {
        return Err("Index CRC mismatch".into());
    }
    if let Some(expected_hmac) = &index.hmac {
        if let (Some(pass), Some(salt)) = (password.as_ref(), index.salt) {
            use hmac::{Hmac, Mac};
            use sha2::Sha256 as Sha256Mac;
            type HmacSha256 = Hmac<Sha256Mac>;
            let key = crypto::derive_key_argon2(&pass, &salt);
            // Для проверки HMAC нужно сериализовать индекс с hmac = None,
            // ровно так же, как при вычислении в create_katana_archive.
            let mut idx_no_hmac = index.clone();
            idx_no_hmac.crc32 = 0;
            idx_no_hmac.hmac = None;
            let idx_json_no_hmac = serde_json::to_vec(&idx_no_hmac)?;
            let mut mac = HmacSha256::new_from_slice(&key).expect("HMAC new");
            mac.update(&idx_json_no_hmac);
            if mac.verify_slice(expected_hmac).is_err() {
                return Err("Index HMAC verification failed".into());
            }
        } else {
            return Err("Encrypted archive: password required for HMAC verification".into());
        }
    }

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
    
    // Pre-derive encryption key once for extraction (if encrypted)
    let key_bytes_arc: Option<Arc<[u8; 32]>> = if let (Some(ref pass), Some(ref salt)) = (password.as_ref(), salt_opt.as_ref()) {
        Some(Arc::new(crypto::derive_key_argon2(pass, &salt[..])))
    } else { None };

    // Initialize progress tracker for extraction
    let mut progress_tracker = ProgressTracker::new(shard_count, std::time::Duration::from_millis(50));
    if let Some(callback) = progress_callback {
        progress_tracker.enable_with_callback(callback);
        progress_tracker.set_totals(files_all.len() as u64, total_uncomp, shard_count);
    }
    let progress_tracker = std::sync::Arc::new(std::sync::Mutex::new(progress_tracker));
    
    // --- Verify shard CRC32 before extraction ---
    // use crc32fast::Hasher as Crc32Hasher; // already imported earlier in function
    for shard in &shards {
        let mut file_crc = File::open(archive_path)?;
        file_crc.seek(SeekFrom::Start(shard.offset))?;
        let mut hasher = Crc32Hasher::new();
        let mut remaining = shard.compressed_size;
        let mut buf = vec![0u8; 8 * 1024 * 1024];
        while remaining > 0 {
            let read_sz = std::cmp::min(remaining, buf.len() as u64) as usize;
            let n = file_crc.read(&mut buf[..read_sz])?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
            remaining -= n as u64;
        }
        let calc = hasher.finalize();
        if calc != shard.crc32 {
            return Err(format!("CRC mismatch in shard at offset {} (expected {:08x}, got {:08x})", shard.offset, shard.crc32, calc).into());
        }
    }

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

            let key_arc_cl = key_bytes_arc.clone();
            let error_flag = had_error.clone();
            let wanted_cl = wanted.clone();
            let strip_components_cl = strip_components;
            let progress_tracker_cl = Arc::clone(&progress_tracker);
            
            // Get thread-specific metrics handle for this shard
            let thread_metrics = {
                let tracker = progress_tracker_cl.lock().unwrap();
                tracker.get_thread_metrics(shard_info.file_count % 8) // Distribute across available metrics
            };
            
            // Pass full slice to maintain correct byte positions
            let shard_vec: Vec<FileEntry> = shard_files_slice.to_vec();
            s.spawn(move |_| {
                if let Err(e) = extract_katana_shard_with_progress(
                    &archive_path,
                    &out_root,
                    &shard_info,
                    &shard_vec,
                    &wanted_cl,
                    key_arc_cl.as_deref(),
                    strip_components_cl,
                    thread_metrics,
                ) {
                    eprintln!("[katana] shard extract error: {}", e);
                    error_flag.store(true, Ordering::SeqCst);
                }
                
                // Record shard completion
                {
                    let tracker = progress_tracker_cl.lock().unwrap();
                    tracker.record_shard_completed();
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
    
    // Force final progress emission to 100%
    {
        let tracker = progress_tracker.lock().unwrap();
        tracker.force_completion();
    }
    
    Ok(())
}

use std::collections::HashSet;
use crate::progress::ThreadMetrics;

fn extract_katana_shard(
    archive_path: &Path,
    out_root: &Path,
    shard_info: &ShardInfo,
    files: &[FileEntry],
    wanted: &HashSet<String>,
    key_bytes: Option<&[u8; 32]>,
    strip_components: Option<u32>,
) -> Result<(), Box<dyn Error>> {
    extract_katana_shard_with_progress(
        archive_path, 
        out_root, 
        shard_info, 
        files, 
        wanted, 
        key_bytes, 
        strip_components,
        None
    )
}

fn extract_katana_shard_with_progress(
    archive_path: &Path,
    out_root: &Path,
    shard_info: &ShardInfo,
    files: &[FileEntry],
    wanted: &HashSet<String>,
    key_bytes: Option<&[u8; 32]>,
    strip_components: Option<u32>,
    thread_metrics: Option<Arc<ThreadMetrics>>,
) -> Result<(), Box<dyn Error>> {
    use std::io::{BufWriter, Cursor, Read};
    let mut shard_file = File::open(archive_path)?;
    shard_file.seek(SeekFrom::Start(shard_info.offset))?;

    // Build a reader depending on encryption
    let reader: Box<dyn Read> = if let Some(nc) = shard_info.nonce {
        // --- Encrypted shard: stream decrypt to temp file (low RAM) ---
        let body_size = shard_info
            .compressed_size
            .checked_sub(16)
            .ok_or("shard size too small for tag")?;
        let key = key_bytes.ok_or("Password/key required for encrypted archive")?;

        // Read tag located at end of shard first
        shard_file.seek(SeekFrom::Start(shard_info.offset + body_size))?;
        let mut tag = [0u8; 16];
        shard_file.read_exact(&mut tag)?;

        // Seek back to start of ciphertext body
        shard_file.seek(SeekFrom::Start(shard_info.offset))?;
        // Ciphertext body reader (excluding tag)
        let mut body_reader = (&mut shard_file).take(body_size);

        // Temp file to hold decrypted stream (avoids holding whole Vec).
        // Include a high-resolution timestamp to ensure uniqueness across concurrent extractions.
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp_path = std::env::temp_dir()
            .join(format!("katana_dec_{}_{}.tmp", shard_info.offset, unique));
        {
            let mut tmp_f = BufWriter::new(File::create(&tmp_path)?);
            decrypt_stream_prekey(&mut body_reader, &mut tmp_f, key, &nc, &tag)
                .map_err(|e| format!("decrypt failed: {:?}", e))?;
            tmp_f.flush()?;
        }
        // Ensure cleanup afterwards
        let cleanup = tmp_path.clone();
        scopeguard::defer! { fs::remove_file(&cleanup).ok(); }
        let opened = File::open(&tmp_path)?;
        Box::new(opened)
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
            // Determine if original path was absolute (Unix /... or Windows C:\...)
            let original_absolute = entry.path.starts_with('/') || (entry.path.len() >= 2 && entry.path.chars().nth(1) == Some(':'));
            // Write this file to disk
            // Ensure path is relative, remove leading slash or drive letter if present
            let mut normalized_path = entry.path.clone();
            
            // Обработка Unix-style путей с /
            if normalized_path.starts_with('/') {
                normalized_path = normalized_path.trim_start_matches('/').to_string();
            }
            
            // Обработка Windows-style путей с C:\, D:\ и т.д.
            #[cfg(windows)]
            {
                // Проверяем на Windows-путь с буквой диска (C:\path\file)
                if normalized_path.len() >= 2 && normalized_path.chars().nth(1) == Some(':') {
                    // Удаляем имя диска и первый разделитель
                    if normalized_path.len() >= 3 && normalized_path.chars().nth(2) == Some('\\') {
                        normalized_path = normalized_path.chars().skip(3).collect::<String>();
                    } else {
                        normalized_path = normalized_path.chars().skip(2).collect::<String>();
                    }
                    
                    // Заменяем обратные слеши на прямые для совместимости
                    normalized_path = normalized_path.replace('\\', "/");
                }
                
                // Если путь начинается с \\
                if normalized_path.starts_with('\\') {
                    normalized_path = normalized_path.trim_start_matches('\\').to_string();
                    normalized_path = normalized_path.replace('\\', "/");
                }
            }
            
            // Если исходный путь был абсолютным – отбросим все промежуточные директории, оставим только имя файла
            if original_absolute {
                if let Some(fname) = std::path::Path::new(&normalized_path).file_name() {
                    normalized_path = fname.to_string_lossy().into_owned();
                }
            }

            // Если путь стал пустым после нормализации (был только /), используем имя файла
            if normalized_path.is_empty() || normalized_path == "/" {
                // Извлечь имя файла из абсолютного пути
                let path = std::path::Path::new(&entry.path);
                if let Some(filename) = path.file_name() {
                    normalized_path = filename.to_string_lossy().into_owned();
                } else {
                    // Если не удалось получить имя файла, смотрим на последний компонент пути
                    let components: Vec<_> = path.components().collect();
                    if let Some(last) = components.last() {
                        normalized_path = last.as_os_str().to_string_lossy().into_owned();
                    } else {
                        // Запасной вариант если ничего не помогло
                        normalized_path = "secret.txt".to_string();
                    }
                }
            }
            
            // Apply strip_components if specified
            if let Some(n) = strip_components {
                let path_buf = std::path::Path::new(&normalized_path).to_path_buf();
                let stripped = crate::extract::strip_path_components(&path_buf, n);
                normalized_path = stripped.to_string_lossy().into_owned();
            }

            // ------------------------------------------------------------------
            // Security hardening: prevent path traversal ("../") and symlink abuse
            // ------------------------------------------------------------------
            // Reject any remaining parent directory components
            if std::path::Path::new(&normalized_path)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                eprintln!("[katana] ⚠️  Skipping suspicious entry with '..': {}", normalized_path);
                // Skip file bytes but continue extraction
                while remaining > 0 {
                    let to_read = std::cmp::min(in_buf.len() as u64, remaining) as usize;
                    let rd = decoder.read(&mut in_buf[..to_read])?;
                    if rd == 0 { return Err("Unexpected EOF while skipping".into()); }
                    remaining -= rd as u64;
                }
                continue;
            }

            let out_path = out_root.join(&normalized_path);

            // Ensure the final canonicalized path is inside output root
            if let (Ok(root_real), Ok(target_real)) = (out_root.canonicalize(), out_path.parent().unwrap_or(out_root).canonicalize()) {
                if !target_real.starts_with(&root_real) {
                    eprintln!("[katana] ⚠️  Detected path escaping output dir: {:?}", out_path);
                    while remaining > 0 {
                        let to_read = std::cmp::min(in_buf.len() as u64, remaining) as usize;
                        let rd = decoder.read(&mut in_buf[..to_read])?;
                        if rd == 0 { return Err("Unexpected EOF while skipping".into()); }
                        remaining -= rd as u64;
                    }
                    continue;
                }
            }
            
            
            if std::env::var("BLITZ_DEBUG_PATHS").is_ok() {
                eprintln!("[dbg] extract -> {:?}", out_path);
            }
            
            // Проверяем, не является ли путь директорией
            if out_path.exists() && (out_path.is_dir() || out_path.symlink_metadata()?.file_type().is_symlink()) {
                // Если это директория, пропускаем этот файл и не пытаемся его создать
                eprintln!("[katana] Warning: skipping file that conflicts with existing directory: {:?}", out_path);
                // Пропускаем данные файла
                while remaining > 0 {
                    let to_read = std::cmp::min(in_buf.len() as u64, remaining) as usize;
                    let rd = decoder.read(&mut in_buf[..to_read])?;
                    if rd == 0 {
                        return Err("Unexpected EOF while skipping".into());
                    }
                    remaining -= rd as u64;
                }
                continue;
            }
            
            // Создаем родительскую директорию если она не существует
            if let Some(dir) = out_path.parent() {
                fs::create_dir_all(dir)?;
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
                // Strip SUID/SGID bits for safety
                let safe_perm = perm & 0o777; // удаляем 0o4000/0o2000
                crate::fsx::set_unix_permissions(&out_path, safe_perm)?;
            }
            
            // Record file extraction (zero-overhead when progress disabled)
            if let Some(ref metrics) = thread_metrics {
                metrics.record_file_processed(entry.size);
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
            
            // Still record progress for skipped files (zero-overhead when progress disabled)
            if let Some(ref metrics) = thread_metrics {
                metrics.record_file_processed(entry.size);
            }
        }
    }
    Ok(())
}
