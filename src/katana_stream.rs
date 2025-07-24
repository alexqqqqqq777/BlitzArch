use std::fs::{File, OpenOptions};
use tempfile::{NamedTempFile, TempPath};

use std::io::{Read, Seek, SeekFrom, Write};

#[cfg(any(unix, target_os = "wasi"))]
use std::os::unix::fs::PermissionsExt; // for mode()
// use of raw fd not required in hybrid stream variant
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::error::Error;


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
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<[u8; 12]>,
}


const KATANA_MAGIC: &[u8; 8] = b"KATIDX01";

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

pub fn create_katana_archive(
    inputs: &[PathBuf],
    output_path: &Path,
    threads: usize,
    mut codec_threads: u32,
    level: i32,
    password: Option<String>,
) -> Result<(), Box<dyn Error>> {
    // ---------------- Adaptive codec threads by memory budget -----------------
    // If codec_threads == 0, interpret as "auto under memory budget".
    if codec_threads == 0 {
        const MAX_INFLIGHT_LOCAL: u64 = MAX_INFLIGHT as u64;
        // read memory budget from env (MiB). "0" or missing -> unlimited
        let budget_bytes = std::env::var("BLITZ_MEM_BUDGET_MB")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|&mb| mb > 0)
            .map(|mb| mb * 1024 * 1024)
            .unwrap_or(u64::MAX);

        // simple heuristic: est mem per thread depends on level
        let est_mem_per_thread = match level {
            0..=4 => 32 * 1024 * 1024u64,    // 32 MiB
            5..=8 => 64 * 1024 * 1024u64,    // 64 MiB
            _ => 128 * 1024 * 1024u64,       // 128 MiB
        } + 2 * 1024 * 1024u64; // +input buffer

        // consider global inflight factor and extra 8 MiB buffer
        let mem_per_shard_threads = est_mem_per_thread * MAX_INFLIGHT_LOCAL;
        let mut max_threads_by_mem = if budget_bytes == u64::MAX {
            num_cpus::get() as u32
        } else {
            ((budget_bytes.saturating_sub(8 * 1024 * 1024)) / mem_per_shard_threads) as u32
        };
        if max_threads_by_mem == 0 { max_threads_by_mem = 1; }
        let cpu_cores = num_cpus::get() as u32;
        codec_threads = std::cmp::min(max_threads_by_mem, cpu_cores);
        if codec_threads == 0 { codec_threads = 1; }
        // Safety cap: don't spawn more than 8 codec threads – diminishing returns
        codec_threads = codec_threads.min(8);
    }
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
            for e in WalkDir::new(path) {
                let e = e?;
                if e.file_type().is_file() {
                    files.push(e.path().to_path_buf());
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

    // 3. Открыть выходной файл
    let mut out_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(output_path)?;

    // 4. Каналы для обмена
    let (tx, rx): (Sender<ShardMsg>, Receiver<ShardMsg>) = bounded(MAX_INFLIGHT);

    // 5. Состояние координатора
    let mut index_shards: Vec<ShardInfo> = Vec::with_capacity(num_shards);
    let mut index_files: Vec<FileEntry> = Vec::new();
    // Temporary storage to keep deterministic order
    let mut shard_infos: Vec<Option<ShardInfo>> = vec![None; num_shards];
    let mut files_by_shard: Vec<Option<Vec<FileEntry>>> = vec![None; num_shards];
    

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
                        let mut encoder = zstd::Encoder::new(&mut sink, level).expect("enc");
                        encoder.include_checksum(true).expect("chk");
                        if zstd_threads > 1 {
                            encoder.multithread(zstd_threads).expect("mt");
                        }
                        let mut in_buf = vec![0u8; 2 << 20]; // 2 MiB
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
                    let mut encoder = zstd::Encoder::new(&mut outfile, level).expect("enc");
                    encoder.include_checksum(true).expect("chk");
                    if zstd_threads > 1 {
                            encoder.multithread(zstd_threads).expect("mt");
                        }
                    let mut in_buf = vec![0u8; 2 << 20];
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
                pending[shard_id] = Some((tmp_path, compressed, uncompressed, files, nonce));
            }
        }
        // Все shard'ы готовы – копируем в порядке shard_id
        for sid in 0..num_shards {
            if let Some((path, comp_size, uncomp_size, files, nonce)) = pending[sid].take() {
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

                shard_infos[sid] = Some(ShardInfo {
                    offset: offset as u64,
                    compressed_size: comp_size,
                    uncompressed_size: uncomp_size,
                    file_count: files.len(),
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
    let index_json_size = index_json.len() as u64;

    out_file.write_all(&index_comp)?;
    out_file.write_all(&index_comp_size.to_le_bytes())?;
    out_file.write_all(&index_json_size.to_le_bytes())?;
    out_file.write_all(KATANA_MAGIC)?;

    // --- Final stats & pretty log ---
    let total_comp_size: u64 = index_comp_size
        + index.shards.iter().map(|s| s.compressed_size).sum::<u64>()
        + 24; // footer
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
        "[katana] Archive complete | Files: {} | Shards: {} | Size: {:.2} → {:.2} MiB (ratio {:.2}x) | CRC: on | Time: {:.2}s | ⏩ {:.1} MB/s",
        index.files.len(),
        index.shards.len(),
        total_uncomp_size as f64 / (1024.0 * 1024.0),
        total_comp_size as f64 / (1024.0 * 1024.0),
        ratio,
        duration.as_secs_f64(),
        throughput,
    );

    Ok(())
}
