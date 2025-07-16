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
}

const KATANA_MAGIC: &[u8; 8] = b"KATIDX01";

const FLUSH_SIZE: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_INFLIGHT: usize = 3; // количество буферов в канале

/// Сообщения от воркеров координатору
enum ShardMsg {
    Done {
        shard_id: usize,
        tmp_path: TempPath,
        compressed: u64,
        uncompressed: u64,
        files: Vec<FileEntry>,
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
    codec_threads: u32,
    level: i32,
    password: Option<String>,
) -> Result<(), Box<dyn Error>> {
    // If encryption requested, fallback to non-stream Katana implementation which supports it
    if password.is_some() {
        return crate::katana::create_katana_archive(inputs, output_path, threads, password);
    }
    // --- High-level stats ---
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
            let tx = tx.clone();
            let base_dir: Arc<PathBuf> = Arc::clone(&base_dir);
            s.spawn(move |_| {
                // Временный файл для сжатого выхода этого шарда
                let mut tmp = NamedTempFile::new().expect("tmp");
                let tmp_path = tmp.path().to_path_buf();

                // zstd encoder пишет напрямую в temp-файл
                let zstd_threads: u32 = if codec_threads == 0 { num_cpus::get() as u32 } else { codec_threads };
                let mut encoder = zstd::Encoder::new(tmp.as_file_mut(), level).expect("enc");
                encoder.include_checksum(true).expect("chk");
                encoder.multithread(zstd_threads).expect("mt");

                let mut in_buf = vec![0u8; 2 << 20]; // 2 MiB чтения
                let mut local_files = Vec::new();
                let mut uncompressed: u64 = 0;

                for path in &chunk {
                    let mut f = File::open(path).expect("open");
                    let meta = f.metadata().expect("meta");
                    let rel_path = match path.strip_prefix(base_dir.as_path()) {
                        Ok(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                        _ => path.strip_prefix(std::path::Component::RootDir.as_os_str()).unwrap_or(path).to_path_buf(),
                    };
                    let normalized_path = crate::katana::normalize_path(&rel_path.to_string_lossy());
                    if std::env::var("BLITZ_DEBUG_PATHS").is_ok() {
                        eprintln!("[dbg] normalize_path: {} -> {}", path.display(), normalized_path);
                    }
                    local_files.push(FileEntry {
                        path: normalized_path,
                        size: meta.len(),
                        offset: uncompressed, // record current offset before writing
                        permissions: {
                            #[cfg(unix)]
                            { crate::fsx::maybe_unix_mode(&meta) }
                            #[cfg(not(unix))]
                            { None }
                        },
                    });
                    loop {
                        let rd = f.read(&mut in_buf).expect("read");
                        if rd == 0 {
                            break;
                        }
                        uncompressed += rd as u64;
                        encoder.write_all(&in_buf[..rd]).expect("enc write");
                    }
                }
                // Завершить поток
                encoder.finish().expect("finish");
                // Преобразуем во временный путь, который удалится автоматически при Drop
                let temp_path: TempPath = tmp.into_temp_path();
                let compressed = std::fs::metadata(&temp_path).expect("meta").len();

                tx.send(ShardMsg::Done {
                    shard_id,
                    tmp_path: temp_path,
                    compressed,
                    uncompressed,
                    files: local_files,
                })
                .expect("send end");
            });
        }
        drop(tx);

        // coordinator – собирает данные от воркеров
        let mut pending: Vec<Option<(TempPath, u64, u64, Vec<FileEntry>)>> = (0..num_shards).map(|_| None).collect();
        while let Ok(msg) = rx.recv() {
             let ShardMsg::Done {
                 shard_id,
                 tmp_path,
                 compressed,
                 uncompressed,
                 files,
             } = msg;
            {
                pending[shard_id] = Some((tmp_path, compressed, uncompressed, files));
            }
        }
        // Все shard'ы готовы – копируем в порядке shard_id
        for sid in 0..num_shards {
            if let Some((path, comp_size, uncomp_size, files)) = pending[sid].take() {
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
        shards: Vec<ShardInfo>,
        files: Vec<FileEntry>,
    }

    let index = KatanaIndex {
        shards: index_shards,
        files: index_files,
    };

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
