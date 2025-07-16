//! # BlitzArch Core Library
//! 
//! This crate provides the core functionality for the `blitzarch` archiver.
//! 
//! It is designed to be used by the `blitzarch` command-line application, but its public API
//! can also be used to programmatically create, inspect, and extract `.blz` archives.
//! 
//! ## Key Modules
//! 
//! - [`archive`]: Contains the logic for reading and writing the archive structure.
//! - [`compress`]: Handles data compression using `zstd`.
//! - [`crypto`]: Manages AES-256-GCM encryption and decryption.
//! - [`extract`]: Provides functions for extracting files from an archive.
//! - [`katana`]: Implements the high-performance, parallel-friendly "Katana" archive format.
//! - [`workers`]: Contains the parallel processing logic for multi-threaded operations.
//! 
//! ## Examples
//! 
//! ```no_run
//! // The high-level API is not yet implemented.
//! // Please use the command-line interface.
//! let api_is_ready = false;
//! ```

#![allow(unused_variables, unused_mut, unused_imports, dead_code)]
// This file declares all the modules in the library.

pub mod archive;
pub mod cli;
pub mod common;
pub mod compress;

pub mod crypto;
pub mod daemon;
pub mod extract;
pub mod index;
pub mod error;
pub use error::ArchiverError;

pub mod workers;

pub mod katana;
pub mod katana_stream;

// Cross-platform filesystem wrapper
pub mod fsx;

// Global dictionary cache (POC)
pub mod dict_cache;

// Parallel block decoder for zstd
pub mod zstd_block;
