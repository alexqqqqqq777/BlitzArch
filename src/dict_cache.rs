//! Simple thread-local dictionary cache (PoC).
//!
//! For the first proof-of-concept we keep **one global dictionary** trained at
//! program start (or first use) from a small sample of the first files seen in
//! the bundle.  The dict is stored in thread-local storage as a raw pointer to
//! `ZSTD_DDict` / `ZSTD_CDict` so that decompressor and compressor workers can
//! reference it without locking.
//!
//! The API purposely minimises scope – enough to experiment in benchmarks.  If
//! the dictionary is not initialised, the helpers fall back to plain zstd.

use std::ptr::NonNull;
use std::sync::OnceLock;

use zstd_sys as zstd;

/// Raw trained dictionary bytes (shared across threads).
static RAW_DICT: OnceLock<Box<[u8]>> = OnceLock::new();

thread_local! {
    /// Thread-local compiled decode dictionary.
    static DDICT: std::cell::Cell<Option<NonNull<zstd::ZSTD_DDict>>> = std::cell::Cell::new(None);
}

/// Provide dictionary bytes to the global cache.
/// Should be called once (e.g. by main thread) before any workers start.
pub fn init(dict_bytes: Box<[u8]>) {
    let _ = RAW_DICT.set(dict_bytes);
}

/// Ensure the current thread has a compiled `ZSTD_DDict` and return its pointer.
///
/// # Safety
/// The returned pointer is valid for the lifetime of the thread.  Caller must
/// *not* free it – it will be freed automatically on thread teardown.
pub unsafe fn get_ddict() -> Option<NonNull<zstd::ZSTD_DDict>> {
    if RAW_DICT.get().is_none() {
        return None; // dictionary not initialised
    }

    DDICT.with(|slot| {
        if let Some(ptr) = slot.get() {
            return Some(ptr);
        }

        // Compile dictionary for this thread.
        let raw = zstd::ZSTD_createDDict(RAW_DICT.get().unwrap().as_ptr() as *const _, RAW_DICT.get().unwrap().len());
        let ptr = NonNull::new(raw)?;
        slot.set(Some(ptr));
        Some(ptr)
    })
}

// Automatically free per-thread DDict on thread exit.
struct DDictDrop(NonNull<zstd::ZSTD_DDict>);
impl Drop for DDictDrop {
    fn drop(&mut self) {
        unsafe {
            zstd::ZSTD_freeDDict(self.0.as_ptr());
        }
    }
}

// Attach a destructor to the thread-local slot.
thread_local! {
    static _DDICT_DROP: std::cell::RefCell<Option<DDictDrop>> = std::cell::RefCell::new(None);
}

pub fn ensure_thread_dict() {
    unsafe {
        if let Some(ptr) = get_ddict() {
            _DDICT_DROP.with(|cell| {
                if cell.borrow().is_none() {
                    cell.replace(Some(DDictDrop(ptr)));
                }
            });
        }
    }
}
