//! Cross-platform filesystem wrapper.
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
