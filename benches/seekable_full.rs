use std::fs::{self, File};
use std::error::Error;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use walkdir::WalkDir;
use zstd_seekable::{Seekable, SeekableCStream};
use zstd_sys::ZSTD_FRAMEHEADERSIZE_MAX;

fn main() -> Result<(), Box<dyn Error>> {
    let dataset = Path::new("/Users/oleksandr/Desktop/Development/BTSL/DATASET");
    let out_path = Path::new("/tmp/seekable_dataset.zst");

    println!("--- Seekable full dataset compression ---");

    // Create output file
    let mut out_file = File::create(&out_path)?;

    let mut cstream = SeekableCStream::new(3, 128 * 1024)?; // level 3, 128 KB frames
    let mut in_buf = vec![0u8; 8 * 1024 * 1024]; // 8 MB
    let mut out_buf = vec![0u8; ZSTD_FRAMEHEADERSIZE_MAX as usize * 2];

    let start_compress = Instant::now();
    let mut total_src = 0u64;
    for entry in WalkDir::new(dataset).into_iter().filter_map(Result::ok).filter(|e| e.file_type().is_file()) {
        let path = entry.path();
        let mut file = File::open(path)?;
        loop {
            let read = file.read(&mut in_buf)?;
            if read == 0 {
                break;
            }
            total_src += read as u64;
            let mut consumed = 0;
            while consumed < read {
                let (written, used) = cstream.compress(&mut out_buf, &in_buf[consumed..read])?;
                out_file.write_all(&out_buf[..written])?;
                consumed += used;
            }
        }
    }
    // finish stream
    loop {
        let written = cstream.end_stream(&mut out_buf)?;
        if written == 0 {
            break;
        }
        out_file.write_all(&out_buf[..written])?;
    }
    out_file.flush()?;
    let compress_dur = start_compress.elapsed();
    let archive_size = out_file.metadata()?.len();

    println!("Compression done in {:?}", compress_dur);
    println!("Source {:.2} GiB -> Archive {:.2} GiB (ratio {:.2}x)",
        total_src as f64 / 1_073_741_824.0,
        archive_size as f64 / 1_073_741_824.0,
        total_src as f64 / archive_size as f64);

    // Decompress to /dev/null to measure speed
    let archive_bytes = fs::read(&out_path)?;
    let mut seekable = Seekable::init_buf(&archive_bytes)?;
    let frames = seekable.get_num_frames();
    let start_decomp = Instant::now();
    let mut sink = io::sink();
    for i in 0..frames {
        let size = seekable.get_frame_decompressed_size(i);
        let mut frame_buf = vec![0u8; size];
        seekable.decompress_frame(&mut frame_buf, i);
        sink.write_all(&frame_buf)?;
    }
    let decomp_dur = start_decomp.elapsed();
    println!("Decompression of {} frames done in {:?}", frames, decomp_dur);

    Ok(())
}
