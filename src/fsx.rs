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

