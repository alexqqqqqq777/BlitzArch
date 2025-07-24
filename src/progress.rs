//! Progress tracking system for BlitzArch archive operations
//! 
//! This module provides zero-overhead progress tracking for multithreaded
//! archive creation and extraction operations.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Per-thread metrics to avoid contention between worker threads
pub struct ThreadMetrics {
    /// Number of files processed by this thread
    pub files_processed: AtomicU64,
    /// Total bytes processed by this thread  
    pub bytes_processed: AtomicU64,
}

impl ThreadMetrics {
    pub fn new() -> Self {
        Self {
            files_processed: AtomicU64::new(0),
            bytes_processed: AtomicU64::new(0),
        }
    }
    
    /// Record a file as processed (zero-overhead atomic increment)
    pub fn record_file_processed(&self, file_size: u64) {
        self.files_processed.fetch_add(1, Ordering::Relaxed);
        self.bytes_processed.fetch_add(file_size, Ordering::Relaxed);
    }
    
    pub fn get_files_processed(&self) -> u64 {
        self.files_processed.load(Ordering::Relaxed)
    }
    
    pub fn get_bytes_processed(&self) -> u64 {
        self.bytes_processed.load(Ordering::Relaxed)
    }
}

/// Current progress state aggregated from all threads
#[derive(Debug, Clone)]
pub struct ProgressState {
    pub total_files: u64,
    pub processed_files: u64,
    pub total_bytes: u64,
    pub processed_bytes: u64,
    pub completed_shards: u32,
    pub total_shards: u32,
    pub elapsed_time: Duration,
    pub speed_mbps: f32,
    pub progress_percent: f32,
}

impl ProgressState {
    /// Calculate estimated time remaining based on current speed
    pub fn estimated_time_remaining(&self) -> Duration {
        if self.speed_mbps <= 0.0 {
            return Duration::from_secs(0);
        }
        
        let remaining_bytes = self.total_bytes.saturating_sub(self.processed_bytes);
        let remaining_mb = remaining_bytes as f32 / (1024.0 * 1024.0);
        let remaining_seconds = remaining_mb / self.speed_mbps;
        
        Duration::from_secs_f32(remaining_seconds.max(0.0))
    }
}

/// Progress callback function type
pub type ProgressCallback = dyn Fn(ProgressState) + Send + Sync;

/// Main progress tracker for archive operations
pub struct ProgressTracker {
    /// Whether progress tracking is enabled
    enabled: bool,
    /// Per-thread metrics to avoid contention
    thread_metrics: Vec<Arc<ThreadMetrics>>,
    /// Total expected metrics
    total_files: AtomicU64,
    total_bytes: AtomicU64,
    total_shards: AtomicUsize,
    completed_shards: AtomicUsize,
    /// Timing
    start_time: Instant,
    last_emit_time: std::sync::Mutex<Instant>,
    emit_interval: Duration,
    /// Progress callback
    callback: Option<Arc<ProgressCallback>>,
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new(num_threads: usize, emit_interval: Duration) -> Self {
        let mut thread_metrics = Vec::with_capacity(num_threads);
        for _ in 0..num_threads {
            thread_metrics.push(Arc::new(ThreadMetrics::new()));
        }
        
        Self {
            enabled: false,
            thread_metrics,
            total_files: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            total_shards: AtomicUsize::new(0),
            completed_shards: AtomicUsize::new(0),
            start_time: Instant::now(),
            last_emit_time: std::sync::Mutex::new(Instant::now()),
            emit_interval,
            callback: None,
        }
    }
    
    /// Enable progress tracking with a callback
    pub fn enable_with_callback<F>(&mut self, callback: F) 
    where
        F: Fn(ProgressState) + Send + Sync + 'static,
    {
        self.enabled = true;
        self.callback = Some(Arc::new(callback));
        self.start_time = Instant::now();
        *self.last_emit_time.lock().unwrap() = Instant::now();
    }
    
    /// Disable progress tracking (zero-overhead when disabled)
    pub fn disable(&mut self) {
        self.enabled = false;
        self.callback = None;
    }
    
    /// Set total expected metrics
    pub fn set_totals(&self, files: u64, bytes: u64, shards: usize) {
        if !self.enabled { return; }
        
        self.total_files.store(files, Ordering::Relaxed);
        self.total_bytes.store(bytes, Ordering::Relaxed);
        self.total_shards.store(shards, Ordering::Relaxed);
    }
    
    /// Get thread-specific metrics handle
    pub fn get_thread_metrics(&self, thread_id: usize) -> Option<Arc<ThreadMetrics>> {
        self.thread_metrics.get(thread_id).cloned()
    }
    
    /// Record completion of a shard
    pub fn record_shard_completed(&self) {
        if !self.enabled { return; }
        
        self.completed_shards.fetch_add(1, Ordering::Relaxed);
        self.maybe_emit_progress();
    }
    
    /// Force emit progress update (called periodically)
    pub fn emit_progress(&self) {
        if !self.enabled { return; }
        
        let state = self.calculate_progress_state();
        if let Some(ref callback) = self.callback {
            callback(state);
        }
    }
    
    /// Force completion and emit final 100% progress
    pub fn force_completion(&self) {
        if !self.enabled { return; }
        
        if let Some(ref callback) = self.callback {
            let mut state = self.calculate_progress_state();
            // Force all metrics to completion values for final emission
            state.progress_percent = 100.0;
            state.processed_files = state.total_files;
            state.processed_bytes = state.total_bytes;
            state.completed_shards = state.total_shards;
            callback(state);
        }
    }
    
    /// Emit progress only if enough time has passed
    fn maybe_emit_progress(&self) {
        if !self.enabled { return; }
        
        let now = Instant::now();
        let should_emit = {
            let last_emit = self.last_emit_time.lock().unwrap();
            now.duration_since(*last_emit) >= self.emit_interval
        };
        
        if should_emit {
            self.emit_progress();
        }
    }
    
    /// Calculate current progress state by aggregating all thread metrics
    fn calculate_progress_state(&self) -> ProgressState {
        // Aggregate across all threads
        let (processed_files, processed_bytes) = self.thread_metrics
            .iter()
            .map(|m| (m.get_files_processed(), m.get_bytes_processed()))
            .fold((0u64, 0u64), |(files, bytes), (f, b)| (files + f, bytes + b));
        
        let total_files = self.total_files.load(Ordering::Relaxed);
        let total_bytes = self.total_bytes.load(Ordering::Relaxed);
        let completed_shards = self.completed_shards.load(Ordering::Relaxed) as u32;
        let total_shards = self.total_shards.load(Ordering::Relaxed) as u32;
        
        let elapsed_time = self.start_time.elapsed();
        
        // Calculate speed in MB/s
        let speed_mbps = if elapsed_time.as_secs_f32() > 0.0 {
            let mb_processed = processed_bytes as f32 / (1024.0 * 1024.0);
            mb_processed / elapsed_time.as_secs_f32()
        } else {
            0.0
        };
        
        // Calculate progress percentage (weighted combination)
        let file_progress = if total_files > 0 {
            (processed_files as f32 / total_files as f32) * 100.0
        } else {
            0.0
        };
        
        let byte_progress = if total_bytes > 0 {
            (processed_bytes as f32 / total_bytes as f32) * 100.0
        } else {
            0.0
        };
        
        let shard_progress = if total_shards > 0 {
            (completed_shards as f32 / total_shards as f32) * 100.0
        } else {
            0.0
        };
        
        // Weighted average: bytes (50%), files (30%), shards (20%)
        let progress_percent = (byte_progress * 0.5 + file_progress * 0.3 + shard_progress * 0.2)
            .min(100.0);
        
        ProgressState {
            total_files,
            processed_files,
            total_bytes,
            processed_bytes,
            completed_shards,
            total_shards,
            elapsed_time,
            speed_mbps,
            progress_percent,
        }
    }
    
    /// Get current progress state
    pub fn get_progress_state(&self) -> ProgressState {
        if !self.enabled {
            return ProgressState {
                total_files: 0,
                processed_files: 0,
                total_bytes: 0,
                processed_bytes: 0,
                completed_shards: 0,
                total_shards: 0,
                elapsed_time: Duration::from_secs(0),
                speed_mbps: 0.0,
                progress_percent: 0.0,
            };
        }
        
        self.calculate_progress_state()
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new(num_cpus::get(), Duration::from_millis(100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_thread_metrics() {
        let metrics = ThreadMetrics::new();
        
        metrics.record_file_processed(1024);
        metrics.record_file_processed(2048);
        
        assert_eq!(metrics.get_files_processed(), 2);
        assert_eq!(metrics.get_bytes_processed(), 3072);
    }

    #[test]
    fn test_progress_tracker() {
        let mut tracker = ProgressTracker::new(2, Duration::from_millis(10));
        
        let progress_updates = Arc::new(std::sync::Mutex::new(Vec::new()));
        let updates_clone = Arc::clone(&progress_updates);
        
        tracker.enable_with_callback(move |state| {
            updates_clone.lock().unwrap().push(state.progress_percent);
        });
        
        tracker.set_totals(100, 1024 * 1024, 4);
        
        // Simulate thread 0 processing files
        if let Some(metrics) = tracker.get_thread_metrics(0) {
            metrics.record_file_processed(512 * 1024);  // 50% of bytes
        }
        
        tracker.emit_progress();
        
        let updates = progress_updates.lock().unwrap();
        assert!(!updates.is_empty());
        assert!(updates[0] > 0.0);
    }

    #[test]
    fn test_multithreaded_progress() {
        let tracker = Arc::new(std::sync::Mutex::new(
            ProgressTracker::new(4, Duration::from_millis(1))
        ));
        
        {
            let mut t = tracker.lock().unwrap();
            t.enable_with_callback(|_| {});
            t.set_totals(1000, 1024 * 1024, 4);
        }
        
        let mut handles = vec![];
        
        // Spawn 4 worker threads
        for thread_id in 0..4 {
            let tracker_clone = Arc::clone(&tracker);
            
            let handle = thread::spawn(move || {
                let metrics = {
                    let t = tracker_clone.lock().unwrap();
                    t.get_thread_metrics(thread_id).unwrap()
                };
                
                // Each thread processes 250 files
                for _ in 0..250 {
                    metrics.record_file_processed(1024);
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        let state = {
            let t = tracker.lock().unwrap();
            t.get_progress_state()
        };
        
        assert_eq!(state.processed_files, 1000);
        assert_eq!(state.processed_bytes, 1024 * 1000);
    }
}
