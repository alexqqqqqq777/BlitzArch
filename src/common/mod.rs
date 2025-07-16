//! Common utilities and types module.
// Shared structs, error types, constants, etc.

use serde::{Deserialize, Serialize};

/// Metadata for a single file or directory entry within the archive.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetadata {
    pub absolute_path: std::path::PathBuf,
    pub path: std::path::PathBuf,
    pub size: u64,
    pub permissions: u32,
    pub modified_time: u64, // Unix timestamp
    pub is_dir: bool,
    #[serde(skip)]
    pub dense_hint: Option<bool>,
    // TODO: Add UID/GID, xattr, ACLs, etc.
}
