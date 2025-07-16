//! # Extraction Module
//! 
//! This module implements the core logic for reading archive metadata and extracting files.
//! It provides both a high-level `extract_files` function and lower-level components like `ArchiveReader`
//! for more granular control.

use crate::archive::{ArchiveFooter, ArchiveHeader, ArchiveIndex, HEADER_SIZE, MAGIC_BYTES};
use crate::crypto;
mod parallel;
mod block_pipeline;
mod writer_pool;

use std::collections::{HashMap, HashSet};
use crate::fsx as fs;
use fs::File;
#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt};
#[cfg(unix)]
use fs::Permissions;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::error::Error;

/// A reader for `.blz` archives, responsible for parsing the header, footer, and index.
pub struct ArchiveReader {
    file: File,
}

impl ArchiveReader {
    /// Creates a new `ArchiveReader` from a file handle.
    pub fn new(file: File) -> Result<Self, io::Error> {
        Ok(Self { file })
    }

    /// Reads the archive footer and the main index, returning the deserialized `ArchiveIndex`.
    /// This method performs seeks to the header and footer to validate the archive and locate the index.
    pub fn read_footer_and_index(&mut self) -> Result<ArchiveIndex, Box<dyn std::error::Error>> {
        // 1. Read the entire header block at once
        let mut header_block = vec![0; HEADER_SIZE as usize];
        self.file.read_exact(&mut header_block)?;

        // 2. Verify magic bytes from the start of the block
        if &header_block[..MAGIC_BYTES.len()] != MAGIC_BYTES {
            return Err("Not a valid MicroFusion archive (magic bytes mismatch).".into());
        }

        // 3. Deserialize header from the part after magic bytes
        let json_part = &header_block[MAGIC_BYTES.len()..];
        // Find the end of the JSON string (it's padded with null bytes)
        let json_end = json_part.iter().position(|&b| b == 0).unwrap_or(json_part.len());
        let header_json_slice = &json_part[..json_end];

        let _header: ArchiveHeader = serde_json::from_slice(header_json_slice).map_err(|_e| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid header JSON")
        })?;

        // 4. Seek to the end to find the footer size and magic bytes
        let mut file = &mut self.file;
        file.seek(SeekFrom::End(-16))?;
        let mut footer_metadata = [0u8; 16];
        file.read_exact(&mut footer_metadata)?;
        let (footer_size_bytes, magic_bytes_from_footer) = footer_metadata.split_at(8);

        if magic_bytes_from_footer != MAGIC_BYTES {
            return Err("Not a valid MicroFusion archive (footer magic bytes mismatch).".into());
        }
        let footer_size = u64::from_le_bytes(footer_size_bytes.try_into().unwrap());

        // 5. Read the footer struct
        file.seek(SeekFrom::End(-16 - footer_size as i64))?;
        let mut footer_reader = (&mut file).take(footer_size);
        let footer: ArchiveFooter = serde_json::from_reader(&mut footer_reader)?;

        // 6. Read the index
        self.file.seek(SeekFrom::Start(footer.index_offset))?;
        let mut index_reader = (&mut self.file).take(footer.index_size);
        let index: ArchiveIndex = serde_json::from_reader(&mut index_reader)?;

        Ok(index)
    }
}

/// Lists the contents of an archive to standard output.
///
/// # Arguments
/// * `file` - The archive file to read.
pub fn list_files(file: File) -> Result<(), Box<dyn Error>> {
    let mut reader = ArchiveReader::new(file)?;
    let index = reader.read_footer_and_index()?;

    if index.header.salt.is_some() {
        println!("Archive is encrypted.");
    }

    println!("Archive Index ({} files):", index.entries.len());
    for entry in index.entries {
        println!("- {} ({} bytes)", entry.path.display(), entry.uncompressed_size);
    }

    Ok(())
}

/// Extracts files from an archive.
///
/// This is the main entry point for the extraction process. It handles both encrypted and unencrypted
/// archives and uses parallel processing where possible.
///
/// # Arguments
/// * `archive_path` - Path to the `.blz` archive file.
/// * `files_to_extract` - A slice of specific file paths to extract. If empty, all files are extracted.
/// * `password` - An optional password for decrypting the archive.
/// * `output_dir` - The directory to extract files to. Defaults to the current working directory.
pub fn extract_files(
    archive_path: &Path,
    files_to_extract: &[PathBuf],
    password: Option<&str>,
    output_dir: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
        // Detect and delegate to Katana extractor if needed
    if crate::katana::is_katana_archive(archive_path)? {
        let base_output_path = match output_dir {
            Some(p) => p.to_path_buf(),
            None => std::env::current_dir()?,
        };
        return crate::katana::extract_katana_archive_internal(
            archive_path,
            &base_output_path,
            files_to_extract,
            password.map(|s| s.to_string()),
        );
    }

    let file = File::open(archive_path)?;
    let mut reader = ArchiveReader::new(file)?;
    let index = reader.read_footer_and_index()?;

    let salt = index.header.salt.as_deref();
    if salt.is_some() && password.is_none() {
        return Err("Archive is encrypted, but no password was provided.".into());
    }

    let base_output_path = match output_dir {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir()?,
    };
    fs::create_dir_all(&base_output_path)?;

    let files_to_extract_set: HashSet<_> = files_to_extract.iter().collect();
    let all_files = files_to_extract.is_empty();

    for entry in &index.entries {
        if entry.is_dir {
            let target_path = base_output_path.join(&entry.path);
            fs::create_dir_all(&target_path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.permissions {
                    crate::fsx::set_unix_permissions(&target_path, mode)?;
                }
            }
        }
    }

    let mut files_by_bundle: HashMap<u32, Vec<_>> = HashMap::new();
    for entry in &index.entries {
        if !entry.is_dir && (all_files || files_to_extract_set.contains(&entry.path)) {
            files_by_bundle
                .entry(entry.bundle_id)
                .or_insert_with(Vec::new)
                .push(entry.clone());
        }
    }

    // Pre-sort files within each bundle to avoid doing it in the loop.
    for files in files_by_bundle.values_mut() {
        files.sort_by_key(|f| f.offset_in_bundle);
    }

    // Determine desired parallelism
    

    // --- Parallel extraction branch (unencrypted only) ---
    if salt.is_none() && password.is_none() {
        use rayon::prelude::*;
        use std::sync::Arc;

        // Prepare list of (bundle_id, files) tasks.
        let tasks: Vec<(u32, Vec<crate::archive::FileIndexEntry>)> = files_by_bundle
            .into_iter()
            .map(|(bid, list)| (bid, list))
            .collect();

        let rayon_threads = rayon::current_num_threads();

        // If only a single Rayon thread is active, there is no point in spawning parallel tasks.
        if rayon_threads <= 1 {
            // Sequential extraction path over the same tasks.
            for (bundle_id, files) in tasks {
                let bundle_info = &index.bundles[bundle_id as usize];
                parallel::extract_bundle_sequential(
                    archive_path,
                    bundle_info,
                    &files,
                    &index,
                    &base_output_path,
                )?;
            }
            return Ok(());
        }

        // --- Real parallel extraction ---
        let index_arc = Arc::new(index);
        let base_out = Arc::new(base_output_path.clone());
        let archive_path_buf = archive_path.to_path_buf();

        use std::io;
        tasks.par_iter().try_for_each(|(bundle_id, files)| -> io::Result<()> {
            let bundle = &index_arc.bundles[*bundle_id as usize];
            parallel::extract_bundle_parallel(
                &archive_path_buf,
                bundle,
                files,
                &index_arc,
                &base_out,
            )
        })?;

        return Ok(());
    }

    // Helper function to extract files from a given zstd decoder stream.
    // The decoder must be based on a `BufRead` for `.take()` to be available.
    fn extract_from_decoder(
        decoder: &mut dyn Read,
        files: &[crate::archive::FileIndexEntry],
        base_output_path: &Path,
        algo: &str,
    ) -> io::Result<()> {
        for file_entry in files {
            let target_path = base_output_path.join(&file_entry.path);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut output_file = File::create(&target_path)?;

            if algo == "store" {
                // For 'store' mode, we read the size prefix for each file.
                let mut size_buf = [0u8; 8];
                decoder.read_exact(&mut size_buf)?;
                let file_size = u64::from_le_bytes(size_buf);

                let mut limited_reader = decoder.take(file_size);
                io::copy(&mut limited_reader, &mut output_file)?;
            } else {
                // For compressed files, the whole bundle is decompressed as a single stream.
                // We rely on the uncompressed_size from the index to read the correct amount of data.
                // --- Read preprocessing sentinel + optional meta block ---
                let mut len_buf = [0u8; 4];
                decoder.read_exact(&mut len_buf)?;
                let meta_len = u32::from_le_bytes(len_buf);

                if meta_len != u32::MAX {
                    // Skip meta block before actual file contents
                    io::copy(&mut (&mut *decoder).take(meta_len as u64), &mut io::sink())?;
                }

                // Now copy exactly `uncompressed_size` bytes of real file data.
                io::copy(&mut decoder.take(file_entry.uncompressed_size), &mut output_file)?;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file_entry.permissions {
                    crate::fsx::set_unix_permissions(&target_path, mode)?;
                }
            }
        }
        Ok(())
    }

    for (&bundle_id, files) in &files_by_bundle {
        let bundle_info = &index.bundles[bundle_id as usize];
        reader.file.seek(SeekFrom::Start(bundle_info.offset))?;

        if let (Some(s), Some(nonce), Some(pass)) = (salt, &bundle_info.nonce, password) {
            // ENCRYPTED: Read the full bundle into memory for decryption.
            let mut raw_bundle_data = vec![0; bundle_info.compressed_size as usize];
            reader.file.read_exact(&mut raw_bundle_data)?;

            let compressed_data = crypto::decrypt(&raw_bundle_data, pass, s, nonce)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Decryption failed. Invalid password?"))?;
            
            let compressed_data_reader = std::io::BufReader::new(&compressed_data[..]);
            let mut decoder: Box<dyn Read> = match bundle_info.algo.as_str() {
                "store" => Box::new(compressed_data_reader),
                "lzma2" => Box::new(xz2::read::XzDecoder::new(compressed_data_reader)),
                 _ => { // zstd
                     if let Some(dict) = &index.dictionary {
                         Box::new(zstd::stream::Decoder::with_dictionary(compressed_data_reader, dict)?)
                     } else {
                         Box::new(zstd::stream::Decoder::new(compressed_data_reader)?)
                     }
                 }
             };
            extract_from_decoder(&mut decoder, &files, &base_output_path, &bundle_info.algo)?;
        } else if salt.is_some() {
            return Err("Inconsistent encryption metadata: archive is encrypted, but bundle is not.".into());
        } else {
            // UNENCRYPTED: Stream directly from the file to save memory.
            let bundle_reader = (&mut reader.file).take(bundle_info.compressed_size);
            let buffered_reader = std::io::BufReader::new(bundle_reader);
            let mut decoder: Box<dyn Read> = match bundle_info.algo.as_str() {
                 "store" => Box::new(buffered_reader),
                "lzma2" => Box::new(xz2::read::XzDecoder::new(buffered_reader)),
                 _ => {
                     if let Some(dict) = &index.dictionary {
                         Box::new(zstd::stream::Decoder::with_dictionary(buffered_reader, dict)?)
                     } else {
                         Box::new(zstd::stream::Decoder::new(buffered_reader)?)
                     }
                 }
             };
            extract_from_decoder(&mut decoder, &files, &base_output_path, &bundle_info.algo)?;
        }
    }

    Ok(())
}