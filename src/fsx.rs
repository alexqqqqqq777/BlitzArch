//! Cross-platform filesystem shim used throughout BlitzArch.
//!
//! For now this is a *very* thin wrapper around `std::fs` so the rest of the
//! codebase can simply `use crate::fsx as fs;` and stay platform-agnostic.
//!
//! * On **all** platforms we publicly re-export every symbol from `std::fs` so
//!   things like `fs::File` or `fs::canonicalize` work out of the box.
//! * On Unix we add a helper `set_unix_permissions()` which forwards to
//!   `std::fs::set_permissions()` with `PermissionsExt::from_mode()`.
//! * On Windows (and any non-Unix target) `set_unix_permissions()` is a no-op.
//!
//! This keeps Windows builds happy while still allowing Unix targets to restore
//! original POSIX mode bits when extracting archives.

use std::io;
use std::path::Path;

// Re-export the whole standard fs module so callers can write `fs::File` etc.
pub use std::fs::*;

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
//!
//! For now we expose a *very* thin shim over `std::fs` that compiles on every
//! platform.  All Unix-specific helpers live behind `cfg(unix)` gates so the
//! Windows build stays green.  More advanced behaviour (cap_std, ADS for
//! storing POSIX bits, etc.) can be added incrementally without touching the
//! rest of the codebase.

use std::io;
use std::path::Path;

// On all platforms just re-export std::fs so types like `File`, `OpenOptions`,
// `Permissions`, etc. remain in scope when crate code writes `use crate::fsx::*`.
pub use std::fs::*;

// ----- Unix helpers ---------------------------------------------------------
#[cfg(unix)]
/// Set POSIX permission bits (e.g. 0o755) on `path`.
pub fn set_unix_permissions(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

// ----- Windows stubs --------------------------------------------------------
#[cfg(not(unix))]
/// No-op on non-Unix platforms.
#[inline]
pub fn set_unix_permissions(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}
//!
//! On Unix we transparently re-export std::fs. On Windows the implementation
//! is backed by `cap_std`, and Unix permission bits are stored in an alternate
//! data stream (`":unix_meta"`) so that extracting an archive can restore the
//! original mode when round-tripping between platforms.
//!
//! At the moment only a minimal subset is implemented; additional helpers will
//! be filled in as code is migrated. The goal is to let the rest of BlitzArch
//! import `crate::fsx::*` instead of touching `std::fs` directly, keeping the
//! call-sites identical across OSes.
//!
//! NOTE: All functions currently fall back to `std::fs` so that nothing breaks
//! during the transition. Full Windows handling will be added incrementally.

use std::io;
use std::path::Path;

#[cfg(not(target_os = "windows"))]
pub use std::fs::*;

#[cfg(not(target_os = "windows"))]
/// Set POSIX permission bits on Unix.
pub fn set_unix_permissions(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(target_os = "windows")]
// Minimal Windows stub – just re-export the standard library so the code compiles.
// A richer cap_std-based implementation can be reintroduced later.
pub use std::fs::*;

#[cfg(target_os = "windows")]
use super::*;

#[cfg(target_os = "windows")]
/// No-op on Windows: POSIX permission bits are not preserved.
pub fn set_unix_permissions(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}
