use std::path::PathBuf;

use std::time::SystemTimeError;

/// The primary error type for all operations in the `blitzarch` crate.
#[derive(Debug)]
pub enum ArchiverError {
    /// An I/O error occurred, typically while reading or writing a file.
    /// Includes the path where the error happened.
    Io { source: std::io::Error, path: PathBuf },

    /// An error occurred when trying to strip a prefix from a file path.
    StripPrefix { prefix: PathBuf, path: PathBuf },

    /// A general cryptographic error, often related to password derivation or key handling.
    Crypto(String),

    /// An error from the underlying `aes-gcm` crate during encryption or decryption.
    AesGcm(aes_gcm::Error),

    /// An error during serialization or deserialization of the archive index.
    SerdeJson(serde_json::Error),

    /// A system time error, which can occur when reading file metadata.
    SystemTime(SystemTimeError),

    /// A wrapper for any other error that doesn't fit the specific variants.
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ArchiverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiverError::Io { source, path } => write!(f, "I/O error on path '{}': {}", path.display(), source),
            ArchiverError::StripPrefix { prefix, path } => write!(f, "Could not strip prefix '{}' from path '{}'", prefix.display(), path.display()),
                        ArchiverError::Crypto(msg) => write!(f, "Crypto error: {}", msg),
            ArchiverError::AesGcm(e) => write!(f, "AEAD encryption error: {}", e),
            ArchiverError::SerdeJson(e) => write!(f, "Serialization error: {}", e),
            ArchiverError::SystemTime(e) => write!(f, "System time error: {}", e),
            ArchiverError::Other(e) => write!(f, "An unexpected error occurred: {}", e),
        }
    }
}

impl std::error::Error for ArchiverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ArchiverError::Io { source, .. } => Some(source),
            ArchiverError::SerdeJson(e) => Some(e),
            ArchiverError::SystemTime(e) => Some(e),
            ArchiverError::Other(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for ArchiverError {
    fn from(err: serde_json::Error) -> Self {
        ArchiverError::SerdeJson(err)
    }
}

impl From<SystemTimeError> for ArchiverError {
    fn from(err: SystemTimeError) -> Self {
        ArchiverError::SystemTime(err)
    }
}

// Generic IO error conversion that doesn't require a path
impl From<aes_gcm::Error> for ArchiverError {
    fn from(err: aes_gcm::Error) -> Self {
        ArchiverError::AesGcm(err)
    }
}

impl From<std::io::Error> for ArchiverError {
    fn from(err: std::io::Error) -> Self {
        ArchiverError::Io { source: err, path: PathBuf::new() } // Generic path
    }
}
