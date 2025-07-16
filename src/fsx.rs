// Cross-platform filesystem shim used throughout BlitzArch.
//
// This is a thin wrapper around `std::fs` that provides platform-agnostic
// filesystem operations. It allows the rest of the codebase to use filesystem
// functions without worrying about platform-specific details.
//
// * On Unix platforms, we provide helpers for handling POSIX permissions.
// * On Windows (and other non-Unix platforms), these helpers are no-ops.

use std::io;
use std::path::Path;

// We no longer re-export the standard fs module directly to avoid import conflicts.
// Instead, callers should explicitly import what they need from std::fs.

/// Return POSIX mode bits if available (Unix), otherwise 0.
#[inline]
pub fn unix_mode(meta: &std::fs::Metadata) -> u32 {
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; meta.permissions().mode() }
    #[cfg(not(unix))]
    { 0 }
}

/// Return Some(mode) on Unix, None on non-Unix.
#[inline]
pub fn maybe_unix_mode(meta: &std::fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    { Some(unix_mode(meta)) }
    #[cfg(not(unix))]
    { None }
}

// --------------------------------------------------------------------------
// Unix-specific helper
// --------------------------------------------------------------------------
#[cfg(unix)]
pub fn set_unix_permissions(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

// --------------------------------------------------------------------------
// Non-Unix stub (Windows, wasm, etc.)
// --------------------------------------------------------------------------
#[cfg(not(unix))]
#[inline]
pub fn set_unix_permissions(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}


