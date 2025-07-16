//! # Standard Archive Format
//! 
//! This module defines the structures and logic for the standard, non-Katana `.blz` archive format.
//! It handles the creation of the archive header, footer, file index, and data bundles.

use crate::common::FileMetadata;
use crate::crypto::{self, generate_salt};
use crate::ArchiverError;
use chrono;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use tempfile::NamedTempFile;
use crate::compress::CompressionAlgo;

pub const MAGIC_BYTES: &[u8; 8] = b"MFUSv01\0";
pub const HEADER_SIZE: u64 = 1024;

/// Represents the header of the archive, located at the beginning of the file.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArchiveHeader {
    /// The version of the archive format.
    pub version: u16,
    /// The Unix timestamp of when the archive was created.
    pub creation_timestamp: i64,
    /// The total number of files and directories in the archive.
    pub file_count: u64,
    /// The salt used for password-based key derivation (PBKDF2). Present only if the archive is encrypted.
    pub salt: Option<Vec<u8>>,
}

/// Represents a single entry (file or directory) in the archive's central index.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileIndexEntry {
    /// The relative path of the file or directory.
    pub path: PathBuf,
    /// True if this entry represents a directory.
    pub is_dir: bool,
    /// The ID of the data bundle where the file's content is stored. Ignored for directories.
    pub bundle_id: u32,
    /// The byte offset of the file's content within its data bundle. Ignored for directories.
    pub offset_in_bundle: u64,
    /// The size of the data as it is stored in the bundle. This may differ from `uncompressed_size` if a pre-processor was used.
    #[serde(default)]
    pub stored_size: u64,
    /// The original, uncompressed size of the file. Ignored for directories.
    pub uncompressed_size: u64,
    /// The Unix-style permissions of the file or directory, if available.
    pub permissions: Option<u32>,
}

/// Represents the central directory of the archive.
///
/// This structure is serialized to JSON and written near the end of the archive file.
/// It contains all the necessary information to locate and decompress file data.
#[derive(Serialize, Deserialize, Debug)]
pub struct ArchiveIndex {
    #[serde(default = "default_algo")]
    pub compression_algo: String,
    pub header: ArchiveHeader,
    pub entries: Vec<FileIndexEntry>,
    pub bundles: Vec<BundleInfo>,
        /// An optional, shared zstd dictionary. If present, this dictionary was used to compress
    /// all bundles in the archive and must be used to decompress them. Storing it once
    /// in the index is key to improving the compression ratio for datasets with many
    /// similar, small files.
    pub dictionary: Option<Vec<u8>>,
}

/// Represents the footer of the archive, located at the end of the file.
#[derive(Serialize, Deserialize, Debug)]
pub struct ArchiveFooter {
    /// The byte offset where the `ArchiveIndex` begins.
    pub index_offset: u64,
    /// The total size of the serialized `ArchiveIndex`.
    pub index_size: u64,
}

/// Contains metadata for a single data bundle.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BundleInfo {
    /// The byte offset where this bundle's data begins in the archive file.
    pub offset: u64,
    /// The compressed size of the bundle. For encrypted bundles, this is the size of the ciphertext.
    pub compressed_size: u64,
    /// The total uncompressed size of all files within this bundle.
    pub uncompressed_size: u64,
    /// The compression algorithm used for this bundle (e.g., "zstd", "store").
    #[serde(default = "default_algo")]
    pub algo: String,
    /// The nonce used for AES-GCM encryption. Present only if the bundle is encrypted.
    pub nonce: Option<Vec<u8>>,
}

/// A writer responsible for constructing a `.blz` archive.
///
/// This struct manages the state of the archive being created, including writing the header,
/// data bundles, and the final index and footer.
pub struct ArchiveWriter {
    compression_algo: String,
    writer: BufWriter<File>,
    password: Option<String>,
    index: ArchiveIndex,
    header_bytes: Vec<u8>,
    current_offset: u64,
}

fn default_algo() -> String { "zstd".into() }

impl ArchiveWriter {
    /// Set the algorithm tag for the *next* bundle to be written.
    /// This does **not** modify the global `index.compression_algo`,
    /// only the field used for BundleInfo generation.
    pub fn set_current_algo(&mut self, algo: &str) {
        if self.compression_algo != algo {
            self.compression_algo = algo.to_string();
        }
    }


    /// Creates a new `ArchiveWriter`.
    ///
    /// # Arguments
    /// * `output_file` - The file handle to write the archive to.
    /// * `password` - An optional password to encrypt the archive.
    /// * `algo` - The default compression algorithm to use for bundles.
    pub fn new(output_file: File, password: Option<String>, algo: CompressionAlgo) -> Result<Self, ArchiverError> {
        let algo_str: String = match algo {
            CompressionAlgo::Zstd => "zstd".into(),
            CompressionAlgo::Lzma2 { .. } => "lzma2".into(),
            CompressionAlgo::Store => "store".into(),
        };
        let salt = if password.is_some() { Some(generate_salt()) } else { None };
        let header = ArchiveHeader {
            version: 1,
            creation_timestamp: chrono::Utc::now().timestamp(),
            file_count: 0,
            salt,
        };

        let header_bytes = serde_json::to_vec(&header)?;

        // use 8 MiB buffer to reduce syscall overhead during bundle writes
        let writer = BufWriter::with_capacity(8 * 1024 * 1024, output_file);

        Ok(Self {
            writer,
            password,
            compression_algo: algo_str.clone(),
            index: ArchiveIndex {
                compression_algo: algo_str,
                header,
                entries: Vec::new(),
                bundles: Vec::new(),
                dictionary: None,
            },
            header_bytes,
            current_offset: HEADER_SIZE,
        })
    }

    /// Writes the initial, fixed-size archive header to the output file.
    pub fn write_header(&mut self) -> Result<(), std::io::Error> {
        let mut full_header = Vec::with_capacity(HEADER_SIZE as usize);
        full_header.extend_from_slice(MAGIC_BYTES);
        full_header.extend_from_slice(&self.header_bytes);
        // Pad the rest with zeros to match the fixed header size
        if full_header.len() < HEADER_SIZE as usize {
            full_header.resize(HEADER_SIZE as usize, 0);
        }
        // Ensure the header is not longer than the allocated size
        full_header.truncate(HEADER_SIZE as usize);
        self.writer.write_all(&full_header)
    }

    /// Adds a file or directory entry to the central index.
    pub fn add_file_entry(&mut self, path: PathBuf, is_dir: bool, bundle_id: u32, offset_in_bundle: u64, stored_size: u64, uncompressed_size: u64, permissions: Option<u32>) {
        self.index.entries.push(FileIndexEntry {
            path,
            is_dir,
            bundle_id,
            offset_in_bundle,
            stored_size,
            uncompressed_size,
            permissions,
        });
        self.index.header.file_count += 1;
    }

    /// Adds a shared compression dictionary to the archive index.
    pub fn write_dictionary(&mut self, dict_data: &[u8]) -> Result<(), ArchiverError> {
        self.index.dictionary = Some(dict_data.to_vec());
        Ok(())
    }

    /// Write a fully in-memory bundle (legacy path).
    /// Writes a compressed data bundle to the archive.
    ///
    /// This method takes an in-memory buffer, encrypts it if a password is set, and writes it to the file.
    pub fn write_store_bundle(&mut self, mut temp_file: NamedTempFile, files: &[FileMetadata]) -> Result<(), ArchiverError> {
        let uncompressed_size: u64 = files.iter().map(|f| f.size).sum::<u64>() + (files.len() as u64 * 8);

        temp_file.seek(SeekFrom::Start(0))?;

        let (data_to_write, nonce) = if let Some(pass) = &self.password {
            let salt = self.index.header.salt.as_ref().unwrap();
            // Read the entire temp file into memory for encryption
            let mut data = Vec::new();
            temp_file.read_to_end(&mut data)?;
            let (encrypted_data, nonce) = crypto::encrypt(&data, pass, salt)?;
            (encrypted_data, Some(nonce.to_vec()))
        } else {
            // This path should ideally not be taken for store, as it's less efficient.
            // But for correctness, we handle it.
            let mut data = Vec::new();
            temp_file.read_to_end(&mut data)?;
            (data, None)
        };

        self.writer.write_all(&data_to_write)?;
        let compressed_size = data_to_write.len() as u64;

        self.index.bundles.push(BundleInfo {
            offset: self.current_offset,
            compressed_size,
            uncompressed_size,
            algo: "store".to_string(),
            nonce,
        });

        self.current_offset += compressed_size;
        Ok(())
    }

    pub fn write_bundle(&mut self, data: &[u8]) -> Result<(), ArchiverError> {
        let uncompressed_size = data.len() as u64;
        let (data_to_write, nonce) = if let Some(pass) = &self.password {
                        let salt = self.index.header.salt.as_ref().unwrap();
            let (encrypted_data, nonce) = crypto::encrypt(data, pass, salt)?;
            (encrypted_data, Some(nonce.to_vec()))
        } else {
            (data.to_vec(), None)
        };

        self.writer.write_all(&data_to_write)?;
        let compressed_size = data_to_write.len() as u64;

        self.index.bundles.push(BundleInfo {
            offset: self.current_offset,
            compressed_size,
            uncompressed_size,
            algo: self.compression_algo.clone(),
            nonce,
        });

        self.current_offset += compressed_size;
        Ok(())
    }



    pub fn write_bundle_stream<R: std::io::Read>(&mut self, mut reader: R, compressed_size: u64) -> Result<(), ArchiverError> {
        
        let start_offset = self.current_offset;
        if let Some(pass) = &self.password {
            // For now, streaming encryption is unimplemented to keep logic simple
            // Future work: encrypt on the fly with chunked AEAD
            return Err(ArchiverError::Other("Streaming encryption not yet supported".into()));
        }
        use std::io::Read;
        const BUF_SZ: usize = 1 << 20; // 1 MiB
        let mut buf = vec![0u8; BUF_SZ];
        let mut bytes_copied: u64 = 0;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 { break; }
            self.writer.write_all(&buf[..n])?;
            bytes_copied += n as u64;
        }
        if bytes_copied != compressed_size {
            return Err(ArchiverError::Other(format!("Streamed bundle size mismatch: expected {}, got {}", compressed_size, bytes_copied).into()));
        }
        self.index.bundles.push(BundleInfo {
            offset: start_offset,
            compressed_size,
            uncompressed_size: 0,
            algo: self.compression_algo.clone(),
            nonce: None,
        });
        self.current_offset += compressed_size;
        Ok(())
    }

    /// Finalizes the archive by writing the central index and footer.
    ///
    /// This method consumes the writer and must be called to produce a valid archive.
    pub fn finalize(mut self) -> Result<(), ArchiverError> {
        let index_offset = self.current_offset;
        let index_bytes = serde_json::to_vec(&self.index)?;
        self.writer.write_all(&index_bytes)?;

        let footer = ArchiveFooter {
            index_offset,
            index_size: index_bytes.len() as u64,
        };
        let footer_bytes = serde_json::to_vec(&footer)?;
        // Write footer bytes
        self.writer.write_all(&footer_bytes)?;
        // Append footer size (little-endian u64)
        let footer_size_le = (footer_bytes.len() as u64).to_le_bytes();
        self.writer.write_all(&footer_size_le)?;
        // Append magic bytes as trailer marker
        self.writer.write_all(MAGIC_BYTES)?;
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compress::CompressionAlgo;
    use std::io::{Read, Seek, SeekFrom, Write};
    use tempfile::{tempfile, NamedTempFile};

    /// Tests that a new, empty archive can be created and finalized correctly.
    #[test]
    fn test_archive_writer_new_and_finalize_empty() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_archive = tempfile()?;
        let archive_clone = temp_archive.try_clone()?;

        let writer = ArchiveWriter::new(archive_clone, None, CompressionAlgo::Zstd)?;
        writer.finalize()?;

        temp_archive.seek(SeekFrom::Start(0))?;
        let mut written_data = Vec::new();
        temp_archive.read_to_end(&mut written_data)?;

        assert!(written_data.len() > 16, "Archive should have a footer");
        Ok(())
    }

    /// Tests adding a single file to the archive.
    #[test]
    fn test_archive_add_one_file() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_archive_file = tempfile()?;
        let archive_clone = temp_archive_file.try_clone()?;

        // Create a dummy file to add to the archive
        let mut dummy_file = NamedTempFile::new()?;
        let file_content = b"This is a test file for the archive.";
        dummy_file.write_all(file_content)?;

        // Create and finalize the archive
        {
            let mut writer = ArchiveWriter::new(archive_clone, None, CompressionAlgo::Store)?;
            writer.write_header()?;

            // Add the file entry
            let file_path = PathBuf::from("test_file.txt");
            writer.add_file_entry(file_path.clone(), false, 0, 0, file_content.len() as u64, file_content.len() as u64, None);

            // Write the file content as a bundle
            writer.write_bundle(file_content)?;
            writer.finalize()?;
        }

        // --- Verification ---
        temp_archive_file.seek(SeekFrom::Start(0))?;
        let mut written_data = Vec::new();
        temp_archive_file.read_to_end(&mut written_data)?;

        let trailer_offset = written_data.len() - 8;
        assert_eq!(&written_data[trailer_offset..], MAGIC_BYTES);

        let footer_size_offset = written_data.len() - 16;
        let mut footer_size_bytes = [0u8; 8];
        footer_size_bytes.copy_from_slice(&written_data[footer_size_offset..footer_size_offset + 8]);
        let footer_size = u64::from_le_bytes(footer_size_bytes);

        let footer_offset = footer_size_offset - footer_size as usize;
        let footer: ArchiveFooter = serde_json::from_slice(&written_data[footer_offset..footer_size_offset])?;

        let index: ArchiveIndex = serde_json::from_slice(&written_data[footer.index_offset as usize..(footer.index_offset + footer.index_size) as usize])?;

        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.header.file_count, 1);
        assert_eq!(index.entries[0].path.to_str(), Some("test_file.txt"));
        assert_eq!(index.entries[0].uncompressed_size, file_content.len() as u64);
        assert_eq!(index.bundles.len(), 1);

        Ok(())
    }
}

