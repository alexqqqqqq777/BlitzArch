use std::fs::{File, OpenOptions};
use tempfile::{NamedTempFile, TempPath};

use std::io::{Read, Seek, SeekFrom, Write};

#[cfg(any(unix, target_os = "wasi"))]
use std::os::unix::fs::PermissionsExt; // for mode()
// use of raw fd not required in hybrid stream variant
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::error::Error;

// --- crossbeam_channel ---------------------------------------------------
use crossbeam_channel::{bounded, Receiver, Sender};
use num_cpus;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
// --- crypto & hashing ---------------------------------------------------
use aes_gcm_stream::Aes256GcmStreamEncryptor;
use rand::rngs::OsRng;
use rand::RngCore;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use crc32fast::Hasher as Crc32Hasher;

type HmacSha256 = Hmac<Sha256>;

// Local replicas of structs to avoid cross-module visibility hassles
#[derive(Serialize, Deserialize, Debug, Clone)]
struct FileEntry {
    path: String,
    size: u64,
    offset: u64, // uncompressed offset within shard
    permissions: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ShardInfo {
    offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    file_count: usize,
    crc32: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<[u8; 12]>,
}


const KATANA_MAGIC: &[u8; 8] = b"KATIDX01";
// --- Footer integrity --------------------------------------------------
// 16-байтная подпись + 8-байт длина данных + 32-байтный BLAKE3
const FOOTER_MAGIC: &[u8; 16] = b"KATANA_HASH_FOOT"; // 16 bytes
const FOOTER_SIZE: usize = 16 + 8 + 32; // 56 байт

const FLUSH_SIZE: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_INFLIGHT: usize = 3; // количество буферов в канале

// --- Streaming encrypt sink --------------------------------------------
struct EncryptSink<'a> {
    inner: &'a mut File,
    enc: Aes256GcmStreamEncryptor,
    bytes: u64,
    nonce: [u8; 12],
}
impl<'a> EncryptSink<'a> {
    fn new(inner: &'a mut File, key: &[u8; 32], nonce: [u8; 12]) -> Self {
        let enc = Aes256GcmStreamEncryptor::new(*key, &nonce);
        Self { inner, enc, bytes: 0, nonce }
    }
    fn finalize(mut self) -> std::io::Result<([u8; 12], u64)> {
        let (ct_tail, tag) = self.enc.finalize();
        if !ct_tail.is_empty() {
            self.inner.write_all(&ct_tail)?;
            self.bytes += ct_tail.len() as u64;
        }
        self.inner.write_all(&tag)?;
        self.bytes += tag.len() as u64;
        Ok((self.nonce, self.bytes))
    }
}
impl<'a> Write for EncryptSink<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let ct = self.enc.update(buf);
        self.inner.write_all(&ct)?;
        self.bytes += ct.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Сообщения от воркеров координатору
enum ShardMsg {
    Done {
        shard_id: usize,
        tmp_path: TempPath,
        compressed: u64,
        uncompressed: u64,
        files: Vec<FileEntry>,
        nonce: Option<[u8; 12]>,
    },

}

/// Разбить вектор на приблизительно равные под-массивы
fn split_even<T: Clone>(list: &[T], parts: usize) -> Vec<Vec<T>> {
    let mut chunks = Vec::with_capacity(parts);
    let chunk_sz = (list.len() + parts - 1) / parts;
    for c in list.chunks(chunk_sz) {
        chunks.push(c.to_vec());
    }
    chunks
}

/// Основная функция создания архива Katana в «гибрид-стрим» режиме
use std::time::Instant;
use crate::autotune::{AutoTuner, CompressionStats};

pub fn create_katana_archive<F>(
    inputs: &[PathBuf],
    output_path: &Path,
    threads: usize,
    mut codec_threads: u32,
    mem_budget_mb: Option<u64>,
    password: Option<String>,
    compression_level: Option<i32>,
    progress_callback: Option<F>,
) -> Result<(), Box<dyn Error>>
where
    F: Fn(crate::progress::ProgressState) + Send + Sync + 'static,
{
    // NOTE: actual implementation continues below



    // ---------------- Adaptive AutoTuner initialization -----------------
    // Initialize AutoTuner with memory budget
    let memory_budget = mem_budget_mb
        .map(|mb| mb as usize * 1024 * 1024)  // Convert MiB to bytes
        .unwrap_or_else(|| {
            // Default: 70% of available system memory
            use sysinfo::System;
            let mut sys = System::new();
            sys.refresh_memory();
            (sys.total_memory() as f64 * 0.7) as usize
        });
    
    let mut autotune = AutoTuner::new(memory_budget);
    
    // Get initial configuration
    // Получаем конфигурацию от AutoTune
    let mut current_config = autotune.tune(None);
    // Защита от нулевого размера буфера (приводит к пустым шардам)
    if current_config.input_buffer_size == 0 {
        // Минимум 256 КиБ для гарантированного чтения
        current_config.input_buffer_size = 256 * 1024;
    }
    
    // Override parameters with AutoTune recommendations
    if codec_threads == 0 {
        codec_threads = current_config.codec_threads as u32;
    }
    
    // Use passed compression level or fall back to AutoTune's recommendation
    let compression_level = compression_level.unwrap_or(current_config.compression_level);
    
    // Clone config before rayon::scope to avoid borrowing issues
    let config_clone = current_config.clone();
    
    println!("[AutoTune] Initial config: threads={}, codec_threads={}, compression_level={}, estimated_memory={}MB, input_buffer={}KB",
             current_config.thread_count, 
             current_config.codec_threads,
             current_config.compression_level,
             current_config.estimated_total_memory / (1024 * 1024),
             current_config.input_buffer_size / 1024);
    // Подготовка шифрования (генерация соли/ключа) при наличии пароля
    if password.is_some() {
        /* fallback удалён – теперь поддерживаем потоковое шифрование напрямую */
    }
    // --- High-level stats ---
    // Ключ/соль
let (key_opt, salt_opt) = if let Some(ref pwd) = password {
    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_argon2(pwd, &salt);
    (Some(Arc::new(key)), Some(salt))
} else {
    (None, None)
};
let start_ts = Instant::now();
    // 1. Собрать список файлов
    let mut files = Vec::new();
    for path in inputs {
        if path.is_file() {
            files.push(path.clone());
        } else if path.is_dir() {
            for entry in WalkDir::new(path) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }

    if files.is_empty() {
        return Err("No input files".into());
    }

     let num_shards = if threads == 0 { num_cpus::get() } else { threads }.max(1);
    println!(
        "[katana] Compressing {} files with {} shards → {}",
        files.len(), num_shards, output_path.display()
    );

    // 2. Разбить файлы на шарды
    // Determine base directory for relative paths
    // Determine common ancestor directory for all inputs
    let base_dir: Arc<PathBuf> = Arc::new(crate::katana::common_parent(inputs));

    let file_chunks: Vec<Vec<PathBuf>> = split_even(&files, num_shards);

    // 3. Выходной файл откроем позже, после завершения всех воркеров

    // 4. Каналы для обмена
    let (tx, rx): (Sender<ShardMsg>, Receiver<ShardMsg>) = bounded(MAX_INFLIGHT);

    // 5. Состояние координатора
    let mut index_shards: Vec<ShardInfo> = Vec::with_capacity(num_shards);
    let mut index_files: Vec<FileEntry> = Vec::new();
    // Temporary storage to keep deterministic order
    let mut shard_infos: Vec<Option<ShardInfo>> = vec![None; num_shards];
    let mut files_by_shard: Vec<Option<Vec<FileEntry>>> = vec![None; num_shards];
    
    // Progress tracking state
    let total_files = files.len();
    let mut completed_shards = 0;
    let mut processed_files = 0;
    let mut processed_bytes = 0u64;
    let total_bytes: u64 = files.iter()
        .map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0))
        .sum();
    

    // 6. Параллельное сжатие – каждый воркер пишет в temp-файл
    rayon::scope(|s| {
        // workers
        for (shard_id, chunk) in file_chunks.into_iter().enumerate() {
            let key_clone = key_opt.clone();
            let tx = tx.clone();
            let base_dir: Arc<PathBuf> = Arc::clone(&base_dir);
            s.spawn(move |_| {
                // Временный файл для сжатого выхода этого шарда
                let mut tmp = NamedTempFile::new().expect("tmp");
                let tmp_path = tmp.path().to_path_buf();

                let mut outfile = tmp.as_file_mut();
                let mut nonce_opt: Option<[u8; 12]> = None;
                let mut uncompressed: u64 = 0;
                let mut local_files: Vec<FileEntry> = Vec::new();

                // Создаём encoder в двух вариантах
                if let Some(ref key_arc) = key_clone {
                    let mut nonce = [0u8; 12];
                    OsRng.fill_bytes(&mut nonce);
                    nonce_opt = Some(nonce);
                    let mut sink = EncryptSink::new(&mut outfile, &*key_arc, nonce);
                    let zstd_threads: u32 = codec_threads; // 0 ⇒ однопоточный zstd
                    {
                        let mut encoder = zstd::Encoder::new(&mut sink, compression_level).expect("enc");
                        encoder.include_checksum(true).expect("chk");
                        if zstd_threads > 1 {
                            encoder.multithread(zstd_threads).expect("mt");
                        }
                        let mut in_buf = vec![0u8; config_clone.input_buffer_size]; // Adaptive buffer
                        for path in &chunk {
                            let mut f = File::open(path).expect("open");
                            let meta = f.metadata().expect("meta");
                            let rel_path = match path.strip_prefix(base_dir.as_path()) {
                                Ok(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                                _ => path.to_path_buf(),
                            };
                            let normalized_path = crate::katana::normalize_path(&rel_path.to_string_lossy());
                            local_files.push(FileEntry {
                                path: normalized_path,
                                size: meta.len(),
                                offset: uncompressed,
                                permissions: {
                                    #[cfg(unix)] { crate::fsx::maybe_unix_mode(&meta) }
                                    #[cfg(not(unix))] { None }
                                },
                            });
                            loop {
                                let rd = f.read(&mut in_buf).expect("read");
                                if rd == 0 { break; }
                                uncompressed += rd as u64;
                                encoder.write_all(&in_buf[..rd]).expect("enc write");
                            }
                        }
                        encoder.finish().expect("finish");
                    }
                    // finalize encryption tag
                    let (_n, _bytes) = sink.finalize().expect("finalize");
                } else {
                    let zstd_threads: u32 = codec_threads; // 0 ⇒ однопоточный zstd
                    let mut encoder = zstd::Encoder::new(&mut outfile, compression_level).expect("enc");
                    encoder.include_checksum(true).expect("chk");
                    if zstd_threads > 1 {
                            encoder.multithread(zstd_threads).expect("mt");
                        }
                    let mut in_buf = vec![0u8; config_clone.input_buffer_size]; // Adaptive buffer
                    for path in &chunk {
                        let mut f = File::open(path).expect("open");
                        let meta = f.metadata().expect("meta");
                        let rel_path = match path.strip_prefix(base_dir.as_path()) {
                            Ok(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                            _ => path.to_path_buf(),
                        };
                        let normalized_path = crate::katana::normalize_path(&rel_path.to_string_lossy());
                        local_files.push(FileEntry {
                            path: normalized_path,
                            size: meta.len(),
                            offset: uncompressed,
                            permissions: {
                                #[cfg(unix)] { crate::fsx::maybe_unix_mode(&meta) }
                                #[cfg(not(unix))] { None }
                            },
                        });
                        loop {
                            let rd = f.read(&mut in_buf).expect("read");
                            if rd == 0 { break; }
                            uncompressed += rd as u64;
                            encoder.write_all(&in_buf[..rd]).expect("enc write");
                        }
                    }
                    encoder.finish().expect("finish");
                }
                let temp_path: TempPath = tmp.into_temp_path();
                let compressed = std::fs::metadata(&temp_path).expect("meta").len();

                tx.send(ShardMsg::Done {
                    shard_id,
                    tmp_path: temp_path,
                    compressed,
                    uncompressed,
                    files: local_files,
                    nonce: nonce_opt,
                }).expect("send");
            });
        }
        drop(tx);

        // coordinator – собирает данные от воркеров
        let mut pending: Vec<Option<(TempPath, u64, u64, Vec<FileEntry>, Option<[u8; 12]>)>> = (0..num_shards).map(|_| None).collect();
        while let Ok(msg) = rx.recv() {
             let ShardMsg::Done {
                 shard_id,
                 tmp_path,
                 compressed,
                 uncompressed,
                 files,
                 nonce,
             } = msg;
            {
                // Update progress tracking (capture file count before moving)
                let file_count = files.len();
                completed_shards += 1;
                processed_files += file_count;
                processed_bytes += uncompressed;
                
                pending[shard_id] = Some((tmp_path, compressed, uncompressed, files, nonce));
                
                // Call progress callback if provided
                if let Some(ref callback) = progress_callback {
                    let progress_percent = (completed_shards as f64 / num_shards as f64) * 100.0;
                    let elapsed = start_ts.elapsed();
                    let speed_mbps = if elapsed.as_secs_f64() > 0.0 {
                        (processed_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64()
                    } else {
                        0.0
                    };
                    
                    let progress_state = crate::progress::ProgressState {
                        progress_percent: progress_percent as f32,
                        speed_mbps: speed_mbps as f32,
                        processed_files: processed_files as u64,
                        total_files: total_files as u64,
                        processed_bytes,
                        total_bytes,
                        completed_shards: completed_shards as u32,
                        total_shards: num_shards as u32,
                        elapsed_time: elapsed,
                    };
                    
                    callback(progress_state);
                }
            }
        }
        // Все shard'ы готовы – копируем в порядке shard_id
        for sid in 0..num_shards {
            if let Some((path, comp_size, uncomp_size, files, nonce)) = pending[sid].take() {
                // Открываем выходной файл в режиме append
                let mut out_file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .read(true)
                    .open(output_path)
                    .expect("open output for append");
                let offset = out_file.seek(SeekFrom::End(0)).expect("seek end");
                let mut tf = File::open(&path).expect("open temp shard");
                {
                    // large buffered copy (8 MiB)
                    let mut buf = vec![0u8; 8 * 1024 * 1024];
                    loop {
                        let n = tf.read(&mut buf).expect("read shard temp");
                        if n == 0 {
                            break;
                        }
                        out_file.write_all(&buf[..n]).expect("write shard");
                    }
                }

                // Посчитаем CRC32 сжатого шарда
                let mut crc32 = crc32fast::Hasher::new();
                {
                    let mut tf_verify = File::open(&path).expect("open shard for crc");
                    let mut buf_crc = vec![0u8; 8 * 1024 * 1024];
                    loop {
                        let n = tf_verify.read(&mut buf_crc).expect("read for crc");
                        if n == 0 { break; }
                        crc32.update(&buf_crc[..n]);
                    }
                }
                shard_infos[sid] = Some(ShardInfo {
                    offset: offset as u64,
                    compressed_size: comp_size,
                    uncompressed_size: uncomp_size,
                    file_count: files.len(),
                    crc32: crc32.finalize(),
                    nonce: nonce,
                });

                files_by_shard[sid] = Some(files);
            }
        }

    });

    // Consolidate shards in order
    for sid in 0..num_shards {
        if let Some(info) = shard_infos[sid].take() {
            index_shards.push(info);
            if let Some(files) = files_by_shard[sid].take() {
                index_files.extend(files);
            }
        }
    }

    // 7. Записать индекс + футер
    #[derive(Serialize, Deserialize)]
    struct KatanaIndex {
        #[serde(default)]
        crc32: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        hmac: Option<[u8;32]>,
        #[serde(skip_serializing_if = "Option::is_none")]
        salt: Option<[u8;16]>,
        shards: Vec<ShardInfo>,
        files: Vec<FileEntry>,
    }

    let mut index = KatanaIndex {
        crc32: 0,
        hmac: None,
        salt: salt_opt.clone().map(|v| {
            let arr: [u8;16] = v.try_into().expect("salt size");
            arr
        }),
        shards: index_shards,
        files: index_files,
    };

    let index_json = serde_json::to_vec(&index)?;
    // CRC32
    let mut crc = Crc32Hasher::new();
    crc.update(&index_json);
    index.crc32 = crc.finalize();
    // HMAC (если шифрование)
    if let (Some(ref key_arc), Some(_)) = (key_opt, salt_opt.as_ref()) {
        let mut mac = HmacSha256::new_from_slice(&key_arc[..]).expect("hmac new");
        mac.update(&index_json);
        let res = mac.finalize().into_bytes();
        index.hmac = Some(res.into());
    }
    let index_json = serde_json::to_vec(&index)?;
    let mut enc = zstd::Encoder::new(Vec::new(), 3)?;
    enc.include_checksum(true).expect("chk");
    enc.write_all(&index_json)?;
    let index_comp = enc.finish()?;

    let index_comp_size = index_comp.len() as u64;

        // Открываем файл для записи индекса и футера
        let mut out_file = OpenOptions::new()
            .read(true)
            .write(true)
            .append(true)
            .open(output_path)?;
    let index_json_size = index_json.len() as u64;

    out_file.write_all(&index_comp)?;
    out_file.write_all(&index_comp_size.to_le_bytes())?;
    out_file.write_all(&index_json_size.to_le_bytes())?;
    out_file.write_all(KATANA_MAGIC)?;

    // --- Write footer (BLAKE3 over all previous bytes) -----------------
    use std::io::Seek;
    let data_len = out_file.seek(SeekFrom::End(0))?; // длина данных без футера
    out_file.flush()?; // гарантируем запись на диск, данные в page-cache

    // Рассчитываем хэш, читая из того же файла (page-cache ➜ почти бесплатно)
    out_file.seek(SeekFrom::Start(0))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = out_file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    let hash = hasher.finalize();

    // Вернуться в конец и дописать футер
    out_file.seek(SeekFrom::Start(data_len))?;
    out_file.write_all(FOOTER_MAGIC)?;
    out_file.write_all(& (data_len as u64).to_le_bytes())?;
    out_file.write_all(hash.as_bytes())?;

    // --- Final stats & pretty log ---
    let total_comp_size: u64 = index_comp_size
        + index.shards.iter().map(|s| s.compressed_size).sum::<u64>()
        + FOOTER_SIZE as u64;
    let total_uncomp_size: u64 = index.files.iter().map(|f| f.size).sum();
    let ratio = if total_comp_size > 0 {
        total_uncomp_size as f64 / total_comp_size as f64
    } else {
        0.0
    };
    let duration = start_ts.elapsed();
    let throughput = if duration.as_secs_f64() > 0.0 {
        (total_uncomp_size as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64()
    } else {
        0.0
    };
    println!(
        "[katana] Archive complete | Files: {} | Shards: {} | Size: {:.2} → {:.2} MiB (ratio {:.2}x) | BLAKE3: on | Time: {:.2}s | ⏩ {:.1} MB/s",
        index.files.len(),
        index.shards.len(),
        total_uncomp_size as f64 / (1024.0 * 1024.0),
        total_comp_size as f64 / (1024.0 * 1024.0),
        ratio,
        duration.as_secs_f64(),
        throughput,
    );
    println!(
        "[CREATE] [████████████] 100.0% | {}/{} files | {:.1} MB/s | {:.2}s",
        index.files.len(),
        index.files.len(),
        throughput,
        duration.as_secs_f64()
    );

    Ok(())
}

/// Creates a Katana archive with optional progress tracking.
///
/// This thin wrapper delegates to `create_katana_archive` and, если указан
/// `progress_callback`, отправляет финальное событие 100 %.
#[allow(clippy::too_many_arguments)]
pub fn create_katana_archive_with_progress<F>(
    inputs: &[PathBuf],
    output_path: &Path,
    threads: usize,
    codec_threads: u32,
    mem_budget_mb: Option<u64>,
    password: Option<String>,
    compression_level: Option<i32>,
    skip_check: bool,
    progress_callback: Option<F>,
) -> Result<(), Box<dyn Error>>
where
    F: Fn(crate::progress::ProgressState) + Send + Sync + 'static,
{
    // Delegate to main implementation with progress callback
    create_katana_archive(inputs, output_path, threads, codec_threads, mem_budget_mb, password, compression_level, progress_callback)?;

    // Conditional paranoid integrity check (secure by default)
    if !skip_check {
        // Perform paranoid integrity check (default secure behavior)
        if let Err(e) = perform_paranoid_check(output_path) {
            return Err(e);
        }
    } else {
        // User opted out of integrity verification
        println!("[paranoid] Integrity check SKIPPED by user request");
    }



    Ok(())
}
// -----------------------------------------------------------------------------
// Полная проверка целостности: читаем футер, пересчитываем BLAKE3
pub fn perform_paranoid_check(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Read, Seek};
    let mut f = std::fs::File::open(path)?;
    let file_len = f.metadata()?.len();
    if file_len < FOOTER_SIZE as u64 {
        return Err("File too small for footer".into());
    }
    // Читать футер
    f.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))?;
    let mut footer = [0u8; FOOTER_SIZE];
    f.read_exact(&mut footer)?;
    // Проверить MAGIC
    if &footer[..16] != FOOTER_MAGIC {
        return Err("Footer magic mismatch".into());
    }
    let data_len = u64::from_le_bytes(footer[16..24].try_into().unwrap());
    if data_len + FOOTER_SIZE as u64 != file_len {
        return Err("Footer length mismatch".into());
    }
    let stored_hash = &footer[24..];

    // Рассчитать хэш заново
    f.seek(SeekFrom::Start(0))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 8192];
    let mut remaining = data_len;
    while remaining > 0 {
        let to_read = std::cmp::min(remaining, buf.len() as u64) as usize;
        f.read_exact(&mut buf[..to_read])?;
        hasher.update(&buf[..to_read]);
        remaining -= to_read as u64;
    }
    let calc_hash = hasher.finalize();
    if calc_hash.as_bytes() != stored_hash {
        let _ = std::fs::remove_file(path);
        return Err("Paranoid integrity check failed: hash mismatch".into());
    }
    println!("[paranoid] Integrity verified, BLAKE3 = {}", calc_hash.to_hex());
    Ok(())
}
