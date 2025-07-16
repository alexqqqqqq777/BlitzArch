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

#[cfg(target_os = "windows")]
mod win {
    use super::*;
    use cap_std::fs as cfs;

    pub use cfs::{create_dir_all, hard_link, read, read_to_string, remove_dir_all, remove_file, rename, symlink_file as symlink};

    /// Open a file with the requested options.
    pub fn open_with_options(path: &Path, opts: &std::fs::OpenOptions) -> io::Result<cfs::File> {
        let cap_path = cfs::Dir::open_ambient_dir(".", cfs::ambient_authority())?.join(path);
        let mut builder = cfs::OpenOptions::new();
        if opts.get_read() {
            builder.read(true);
        }
        if opts.get_write() {
            builder.write(true);
        }
        if opts.get_append() {
            builder.append(true);
        }
        if opts.get_truncate() {
            builder.truncate(true);
        }
        if opts.get_create() {
            builder.create(true);
        }
        if opts.get_create_new() {
            builder.create_new(true);
        }
        builder.open(&cap_path)
    }

    /// Temporary placeholder that mimics `std::fs::File::open`.
    pub fn open(path: &Path) -> io::Result<cfs::File> {
        open_with_options(path, std::fs::OpenOptions::new().read(true))
    }

    /// Best-effort Unix permissions emulation.
    pub fn set_unix_permissions(_path: &Path, _mode: u32) -> io::Result<()> {
        // TODO: store mode into ADS ":unix_meta" as u32 LE bytes
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub use win::*;
