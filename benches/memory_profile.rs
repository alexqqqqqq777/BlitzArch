#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

use std::fs::File;
use std::io;
use std::path::PathBuf;
use aatrnnbdye::{compress, extract};
use tempfile::tempdir;
use dhat;

fn main() -> io::Result<()> {
    let source_data_path = PathBuf::from("/Users/oleksandr/Desktop/Development/tmp_bench_full/unpack_l7");

    // --- MFA Archiver (Create) ---
    println!("--- Memory Profiling: MFA Archiver (Create) ---");
    let temp_dir_mfa_create = tempdir()?;
    let archive_path_mfa_create = temp_dir_mfa_create.path().join("test_mfa.mfa");
    let profiler_mfa_create = dhat::Profiler::new_heap();
    let options = compress::CompressOptions {
        algo: compress::CompressionAlgo::Zstd,
        level: 3,
        threads: 0,
        preprocess: false,
        text_bundle: aatrnnbdye::cli::TextBundleMode::Small,
        adaptive: false,
        adaptive_threshold: 0.8,
    };
    compress::run(
        &[source_data_path.clone()],
        &archive_path_mfa_create,
        options,
        None,
    ).unwrap();
    drop(profiler_mfa_create);

    // --- tar + zstd (Create) ---
    println!("\n--- Memory Profiling: tar + zstd (Create) ---");
    let temp_dir_tar_create = tempdir()?;
    let archive_path_tar_create = temp_dir_tar_create.path().join("test_tar.tar.zst");
    let profiler_tar_create = dhat::Profiler::new_heap();
    {
        let archive_file = File::create(&archive_path_tar_create)?;
        let mut zstd_encoder = zstd::stream::write::Encoder::new(archive_file, 7)?;
        {
            let mut tar_builder = tar::Builder::new(&mut zstd_encoder);
            tar_builder.append_dir_all(source_data_path.file_name().unwrap(), &source_data_path)?;
            tar_builder.finish()?;
        } // tar_builder is dropped here, releasing the borrow.
        zstd_encoder.finish()?;
    }
    drop(profiler_tar_create);

    // --- MFA Archiver (Extract) ---
    println!("\n--- Memory Profiling: MFA Archiver (Extract) ---");
    let temp_dir_mfa_extract = tempdir()?;
    let extract_path_mfa = temp_dir_mfa_extract.path();
    let profiler_mfa_extract = dhat::Profiler::new_heap();
    extract::extract_files(&archive_path_mfa_create, &[], None, Some(extract_path_mfa)).unwrap();
    drop(profiler_mfa_extract);

    // --- tar + zstd (Extract) ---
    println!("\n--- Memory Profiling: tar + zstd (Extract) ---");
    let temp_dir_tar_extract = tempdir()?;
    let extract_path_tar = temp_dir_tar_extract.path();
    let profiler_tar_extract = dhat::Profiler::new_heap();
    {
        let archive_file = File::open(&archive_path_tar_create)?;
        let zstd_decoder = zstd::stream::read::Decoder::new(archive_file)?;
        let mut archive = tar::Archive::new(zstd_decoder);
        archive.unpack(extract_path_tar)?;
    }
    drop(profiler_tar_extract);

    Ok(())
}
