use std::sync::{Condvar, Mutex, Arc};

/// A very simple blocking pool that keeps the total amount of "borrowed" bytes
/// not exceeding the configured limit.  This is **coarse-grained** control –
/// callers should reserve the *estimated* amount of memory they expect to hold
/// for their work unit (e.g. a shard).  If there is not enough free capacity
/// the caller will block until some memory is returned back to the pool.
///
/// It is purposely kept `Send + Sync` so that an `Arc<PagePool>` can be shared
/// between all worker threads.
#[derive(Debug)]
pub struct PagePool {
    /// Remaining free capacity in bytes
    remaining: Mutex<u64>,
    cv: Condvar,
    total: u64,
}

impl PagePool {
    /// Create a new pool with the given total capacity (bytes).
    pub fn new(total_bytes: u64) -> Arc<Self> {
        Arc::new(PagePool {
            remaining: Mutex::new(total_bytes),
            cv: Condvar::new(),
            total: total_bytes,
        })
    }

    /// Blocks until at least `bytes` can be reserved.
    pub fn acquire(&self, bytes: u64) {
        let mut guard = self.remaining.lock().unwrap();
        while *guard < bytes {
            // Wait until some capacity is released
            guard = self.cv.wait(guard).unwrap();
        }
        *guard -= bytes;
    }

    /// Returns previously reserved bytes back to the pool and wakes waiters.
    pub fn release(&self, bytes: u64) {
        let mut guard = self.remaining.lock().unwrap();
        *guard += bytes;
        // Clamp in case callers release more than they acquired (should not happen)
        if *guard > self.total {
            *guard = self.total;
        }
        self.cv.notify_all();
    }
}
