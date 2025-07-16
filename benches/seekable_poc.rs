use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::time::Instant;

use rayon::prelude::*;
use zstd_seekable::{Seekable, SeekableCStream};
use zstd_sys::ZSTD_FRAMEHEADERSIZE_MAX;

#[derive(Debug)]
enum PocError {
    Io(io::Error),
    Zstd(zstd_seekable::Error),
}

impl From<io::Error> for PocError {
    fn from(err: io::Error) -> Self {
        PocError::Io(err)
    }
}
impl From<zstd_seekable::Error> for PocError {
    fn from(err: zstd_seekable::Error) -> Self {
        PocError::Zstd(err)
    }
}

fn main() -> Result<(), PocError> {
    println!("--- zstd-seekable Proof of Concept ---");

    // 1. Data preparation
    let dataset_base_path = Path::new("/Users/oleksandr/Desktop/Development/BTSL/DATASET");
    let source_files_to_read = vec![dataset_base_path.join("logs/BGL.log")];

    let mut source_data = Vec::new();
    let mut total_size: u64 = 0;
    println!("[1] Reading and concatenating files...");
    for file_path in source_files_to_read {
        match fs::File::open(&file_path) {
            Ok(mut file) => {
                let file_size = file.metadata()?.len();
                total_size += file_size;
                file.read_to_end(&mut source_data)?;
            }
            Err(e) => {
                println!("Warning: Could not read file {}: {}", file_path.display(), e);
            }
        }
    }
    println!("   - Total source data size: {:.2} MB", total_size as f64 / 1_048_576.0);
    if source_data.is_empty() {
        eprintln!("Error: No source data could be read. Aborting.");
        return Ok(());
    }

    let num_threads = num_cpus::get();
    println!("[2] Architecting for parallelism with {} threads...", num_threads);

    // Split data into chunks
    let chunk_size = (source_data.len() + num_threads - 1) / num_threads;
    let source_chunks: Vec<&[u8]> = source_data.chunks(chunk_size).collect();

    // Parallel compression
    println!("[3] Compressing independent archives in parallel...");
    let start_par_compress = Instant::now();
    let compressed_archives: Vec<Result<Vec<u8>, PocError>> = source_chunks
        .par_iter()
        .map(|chunk| {
            let mut cstream = SeekableCStream::new(1, 128 * 1024)?; // level 1
            let mut compressed_data = Vec::with_capacity(chunk.len());
            let mut input_pos = 0;
            let mut out_buf = vec![0u8; ZSTD_FRAMEHEADERSIZE_MAX as usize * 2];
            while input_pos < chunk.len() {
                let (written, read) = cstream.compress(&mut out_buf, &chunk[input_pos..])?;
                compressed_data.extend_from_slice(&out_buf[..written]);
                input_pos += read;
            }
            loop {
                let written = cstream.end_stream(&mut out_buf)?;
                if written == 0 {
                    break;
                }
                compressed_data.extend_from_slice(&out_buf[..written]);
            }
            Ok(compressed_data)
        })
        .collect();
    let par_compress_duration = start_par_compress.elapsed();
    println!("   - Parallel compression finished in: {:?}", par_compress_duration);

    // Parallel decompression
    println!("[4] Decompressing independent archives in parallel...");
    let start_par_decompress = Instant::now();
    let par_decompressed_chunks: Vec<Result<Vec<u8>, PocError>> = compressed_archives
        .into_par_iter()
        .map(|archive_res| {
            let archive = archive_res?;
            let mut seekable = Seekable::init_buf(&archive)?;
            let mut decompressed = Vec::new();
            let frames = seekable.get_num_frames();
            for i in 0..frames {
                let fsize = seekable.get_frame_decompressed_size(i);
                let mut buf = vec![0u8; fsize];
                seekable.decompress_frame(&mut buf, i);
                decompressed.extend(buf);
            }
            Ok(decompressed)
        })
        .collect();
    let par_decompress_duration = start_par_decompress.elapsed();
    let total_par_duration = start_par_compress.elapsed();
    println!("   - Parallel decompression finished in: {:?}", par_decompress_duration);

    // Sequential processing for comparison
    println!("[5] Sequential processing for comparison...");
    let start_seq = Instant::now();
    let mut seq_decompressed = Vec::new();
    for chunk in &source_chunks {
        let mut cstream = SeekableCStream::new(1, 128 * 1024)?;
        let mut comp = Vec::with_capacity(chunk.len());
        let mut input_pos = 0;
        let mut out_buf = vec![0u8; ZSTD_FRAMEHEADERSIZE_MAX as usize * 2];
        while input_pos < chunk.len() {
            let (written, read) = cstream.compress(&mut out_buf, &chunk[input_pos..])?;
            comp.extend_from_slice(&out_buf[..written]);
            input_pos += read;
        }
        loop {
            let written = cstream.end_stream(&mut out_buf)?;
            if written == 0 {
                break;
            }
            comp.extend_from_slice(&out_buf[..written]);
        }
        let mut seekable = Seekable::init_buf(&comp)?;
        let frames = seekable.get_num_frames();
        for i in 0..frames {
            let fsize = seekable.get_frame_decompressed_size(i);
            let mut buf = vec![0u8; fsize];
            seekable.decompress_frame(&mut buf, i);
            seq_decompressed.extend(buf);
        }
    }
    let seq_duration = start_seq.elapsed();

    // Combine parallel chunks
    let mut par_decompressed = Vec::with_capacity(source_data.len());
    for r in par_decompressed_chunks {
        par_decompressed.extend(r?);
    }

    println!("\n--- Results ---");
    println!("Sequential total: {:?}", seq_duration);
    println!("Parallel total:   {:?}", total_par_duration);
    if total_par_duration < seq_duration {
        println!("Speedup: {:.2}x", seq_duration.as_secs_f64() / total_par_duration.as_secs_f64());
    } else {
        println!("No speedup observed.");
    }

    println!("Verifying data integrity...");
    if source_data == seq_decompressed {
        println!("Sequential OK");
    } else {
        println!("Sequential mismatch!");
    }
    if source_data == par_decompressed {
        println!("Parallel OK");
    } else {
        println!("Parallel mismatch!");
    }

    Ok(())
}
