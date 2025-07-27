//! # Adaptive AutoTune for KatanaStream
//! 
//! This module implements intelligent resource management and bottleneck detection
//! to maximize compression throughput within memory budget constraints.

use std::time::{Duration, Instant};
use sysinfo::{System, Pid, Process};

/// Memory budget in bytes
pub type MemoryBudget = usize;

/// The main bottleneck types that can limit performance
#[derive(Debug, Clone, PartialEq)]
pub enum BottleneckType {
    /// I/O bound: disk throughput is the limiting factor
    IOBound,
    /// CPU bound: compression/decompression is the limiting factor  
    CPUBound,
    /// Memory bound: insufficient RAM causes swapping
    MemoryBound,
    /// Fragmented I/O: many small files cause syscall overhead
    FragmentedIO,
    /// Compression limited: compression algorithm is too slow
    CompressionLimited,
    /// Balanced: no clear bottleneck
    Balanced,
}

/// Real-time performance statistics
#[derive(Debug, Clone)]
pub struct RealtimeStats {
    /// CPU utilization percentage (0.0-100.0)
    pub cpu_utilization: f64,
    /// Time spent waiting for I/O as percentage (0.0-100.0)
    pub io_wait_percent: f64,
    /// Memory pressure indicator (0.0-1.0)
    pub memory_pressure: f64,
    /// Page faults per second
    pub page_faults_per_sec: f64,
    /// Swap usage in MB
    pub swap_in_mb: f64,
    /// Average file size being processed
    pub avg_file_size: u64,
    /// System calls per second
    pub syscalls_per_sec: f64,
    /// Compression speed per thread in MB/s
    pub compression_mbps_per_thread: f64,
    /// Current RSS memory usage in bytes
    pub current_memory_usage: u64,
    /// Disk queue depth
    pub disk_queue_depth: f64,
}

/// Configuration for optimal resource allocation
#[derive(Debug, Clone)]
pub struct OptimalConfig {
    /// Number of worker threads
    pub thread_count: usize,
    /// Number of codec threads for compression
    pub codec_threads: usize,
    /// Input buffer size per thread
    pub input_buffer_size: usize,
    /// Compression buffer size per thread
    pub compression_buffer_size: usize,
    /// Output buffer size per thread
    pub output_buffer_size: usize,
    /// Compression level (higher = better ratio, slower)
    pub compression_level: i32,
    /// Whether to enable file batching for small files
    pub enable_file_batching: bool,
    /// Whether to use streaming mode (lower memory)
    pub streaming_mode: bool,
    /// Prefetch factor for read-ahead
    pub prefetch_factor: f64,
    /// Estimated total memory usage
    pub estimated_total_memory: usize,
}

/// Detects performance bottlenecks in real-time
pub struct BottleneckDetector {
    system: System,
    last_update: Instant,
    update_interval: Duration,
    process_id: Option<u32>,
}

impl BottleneckDetector {
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
            last_update: Instant::now(),
            update_interval: Duration::from_millis(500), // Update every 500ms
            process_id: None,
        }
    }

    /// Update system statistics
    pub fn update_stats(&mut self) {
        if self.last_update.elapsed() >= self.update_interval {
            self.system.refresh_all();
            self.last_update = Instant::now();
            
            // Try to get current process ID if not set
            if self.process_id.is_none() {
                self.process_id = Some(std::process::id());
            }
        }
    }

    /// Collect real-time performance statistics
    pub fn collect_stats(&mut self) -> RealtimeStats {
        self.update_stats();
        
        // CPU utilization
        let cpu_utilization = self.system.global_cpu_info().cpu_usage() as f64;
        
        // Memory stats
        let total_memory = self.system.total_memory() as f64;
        let used_memory = self.system.used_memory() as f64;
        let memory_pressure = used_memory / total_memory;
        
        // Process-specific stats if available
        let (current_memory_usage, _page_faults) = if let Some(pid) = self.process_id {
            if let Some(process) = self.system.process(sysinfo::Pid::from(pid as usize)) {
                (process.memory(), 0.0) // page_faults not directly available
            } else {
                (0, 0.0)
            }
        } else {
            (0, 0.0)
        };
        
        RealtimeStats {
            cpu_utilization,
            io_wait_percent: 0.0, // Would need more detailed system monitoring
            memory_pressure,
            page_faults_per_sec: 0.0, // Would need delta tracking
            swap_in_mb: (self.system.total_swap() - self.system.free_swap()) as f64 / (1024.0 * 1024.0),
            avg_file_size: 0, // Will be provided by caller
            syscalls_per_sec: 0.0, // Would need system-level monitoring
            compression_mbps_per_thread: 0.0, // Will be provided by caller
            current_memory_usage: current_memory_usage as u64,
            disk_queue_depth: 1.0, // Simplified
        }
    }

    /// Detect the primary bottleneck based on current statistics
    pub fn detect_bottleneck(&mut self, stats: &RealtimeStats) -> BottleneckType {
        // Memory bound: high memory pressure or swapping
        if stats.memory_pressure > 0.9 || stats.swap_in_mb > 100.0 {
            return BottleneckType::MemoryBound;
        }
        
        // I/O bound: high I/O wait, moderate CPU usage
        if stats.io_wait_percent > 30.0 && stats.cpu_utilization < 70.0 {
            return BottleneckType::IOBound;
        }
        
        // CPU bound: high CPU usage, low I/O wait
        if stats.cpu_utilization > 85.0 && stats.io_wait_percent < 15.0 {
            return BottleneckType::CPUBound;
        }
        
        // Fragmented I/O: many small files
        if stats.avg_file_size < 64 * 1024 && stats.syscalls_per_sec > 5000.0 {
            return BottleneckType::FragmentedIO;
        }
        
        // Compression limited: low compression throughput per thread
        if stats.compression_mbps_per_thread > 0.0 && stats.compression_mbps_per_thread < 25.0 {
            return BottleneckType::CompressionLimited;
        }
        
        // Default to balanced if no clear bottleneck
        BottleneckType::Balanced
    }
}

/// Calculates optimal configuration based on memory budget and bottleneck type
pub struct ResourceCalculator {
    memory_budget: MemoryBudget,
    tolerance: f64, // ±5% = 0.05
}

impl ResourceCalculator {
    pub fn new(memory_budget: MemoryBudget) -> Self {
        Self {
            memory_budget,
            tolerance: 0.05, // ±5%
        }
    }

    /// Calculate optimal configuration for the detected bottleneck
    pub fn calculate_optimal_config(&self, bottleneck: BottleneckType, stats: &RealtimeStats) -> OptimalConfig {
        match bottleneck {
            BottleneckType::IOBound => self.io_bound_strategy(),
            BottleneckType::CPUBound => self.cpu_bound_strategy(),
            BottleneckType::MemoryBound => self.memory_bound_strategy(),
            BottleneckType::FragmentedIO => self.fragmented_io_strategy(),
            BottleneckType::CompressionLimited => self.compression_limited_strategy(),
            BottleneckType::Balanced => self.balanced_strategy(),
        }
    }

    /// Strategy for I/O bound workloads: maximize I/O efficiency
    fn io_bound_strategy(&self) -> OptimalConfig {
        let cpu_cores = num_cpus::get();
        
        // Use fewer threads to avoid random I/O, more memory for buffers
        let thread_count = (cpu_cores / 2).max(1);
        let codec_threads = thread_count;
        
        // Allocate most memory to I/O buffers
        let system_overhead = self.estimate_system_overhead();
        let working_memory = self.memory_budget - system_overhead;
        let memory_per_thread = working_memory / thread_count;
        
        let input_buffer_size = memory_per_thread * 60 / 100; // 60% for input
        let compression_buffer_size = memory_per_thread * 25 / 100; // 25% for compression
        let output_buffer_size = memory_per_thread * 15 / 100; // 15% for output
        
        OptimalConfig {
            thread_count,
            codec_threads,
            input_buffer_size,
            compression_buffer_size,
            output_buffer_size,
            compression_level: 3, // Balanced compression
            enable_file_batching: false,
            streaming_mode: false,
            prefetch_factor: 4.0, // Aggressive prefetch
            estimated_total_memory: working_memory + system_overhead,
        }
    }

    /// Strategy for CPU bound workloads: maximize CPU utilization
    fn cpu_bound_strategy(&self) -> OptimalConfig {
        let cpu_cores = num_cpus::get();
        
        // Use more threads, less memory per thread
        let thread_count = cpu_cores;
        let codec_threads = cpu_cores;
        
        let system_overhead = self.estimate_system_overhead();
        let working_memory = self.memory_budget - system_overhead;
        let memory_per_thread = working_memory / thread_count;
        
        let input_buffer_size = memory_per_thread * 30 / 100; // 30% for input
        let compression_buffer_size = memory_per_thread * 60 / 100; // 60% for compression
        let output_buffer_size = memory_per_thread * 10 / 100; // 10% for output
        
        OptimalConfig {
            thread_count,
            codec_threads,
            input_buffer_size,
            compression_buffer_size,
            output_buffer_size,
            compression_level: 1, // Fast compression
            enable_file_batching: false,
            streaming_mode: false,
            prefetch_factor: 1.0, // Minimal prefetch
            estimated_total_memory: working_memory + system_overhead,
        }
    }

    /// Strategy for memory bound workloads: minimize memory usage
    fn memory_bound_strategy(&self) -> OptimalConfig {
        // Use minimal resources
        let thread_count = 2;
        let codec_threads = 2;
        
        let system_overhead = self.estimate_system_overhead();
        let working_memory = (self.memory_budget - system_overhead) / 2; // Use only half
        let memory_per_thread = working_memory / thread_count;
        
        let input_buffer_size = (4 * 1024 * 1024).min(memory_per_thread / 3); // 4MB max
        let compression_buffer_size = (4 * 1024 * 1024).min(memory_per_thread / 3);
        let output_buffer_size = (2 * 1024 * 1024).min(memory_per_thread / 3);
        
        OptimalConfig {
            thread_count,
            codec_threads,
            input_buffer_size,
            compression_buffer_size,
            output_buffer_size,
            compression_level: 3, // Balanced
            enable_file_batching: false,
            streaming_mode: true, // Enable streaming
            prefetch_factor: 0.5, // Conservative prefetch
            estimated_total_memory: working_memory + system_overhead,
        }
    }

    /// Strategy for fragmented I/O: batch small files
    fn fragmented_io_strategy(&self) -> OptimalConfig {
        let cpu_cores = num_cpus::get();
        
        // Fewer threads for sequential I/O, large batching buffer
        let thread_count = (cpu_cores / 3).max(1);
        let codec_threads = cpu_cores; // But more compression threads
        
        let system_overhead = self.estimate_system_overhead();
        let working_memory = self.memory_budget - system_overhead;
        let memory_per_thread = working_memory / thread_count;
        
        let input_buffer_size = memory_per_thread * 70 / 100; // Large batching buffer
        let compression_buffer_size = memory_per_thread * 20 / 100;
        let output_buffer_size = memory_per_thread * 10 / 100;
        
        OptimalConfig {
            thread_count,
            codec_threads,
            input_buffer_size,
            compression_buffer_size,
            output_buffer_size,
            compression_level: 3,
            enable_file_batching: true, // Enable batching
            streaming_mode: false,
            prefetch_factor: 2.0,
            estimated_total_memory: working_memory + system_overhead,
        }
    }

    /// Strategy for compression limited workloads: optimize compression
    fn compression_limited_strategy(&self) -> OptimalConfig {
        let cpu_cores = num_cpus::get();
        
        let thread_count = cpu_cores;
        let codec_threads = cpu_cores * 2; // More compression threads
        
        let system_overhead = self.estimate_system_overhead();
        let working_memory = self.memory_budget - system_overhead;
        let memory_per_thread = working_memory / thread_count;
        
        let input_buffer_size = memory_per_thread * 25 / 100;
        let compression_buffer_size = memory_per_thread * 65 / 100; // Large compression buffer
        let output_buffer_size = memory_per_thread * 10 / 100;
        
        OptimalConfig {
            thread_count,
            codec_threads,
            input_buffer_size,
            compression_buffer_size,
            output_buffer_size,
            compression_level: -1, // Fastest compression
            enable_file_batching: false,
            streaming_mode: false,
            prefetch_factor: 1.5,
            estimated_total_memory: working_memory + system_overhead,
        }
    }

    /// Balanced strategy when no clear bottleneck is detected
    fn balanced_strategy(&self) -> OptimalConfig {
        let cpu_cores = num_cpus::get();
        
        let thread_count = cpu_cores;
        let codec_threads = cpu_cores;
        
        let system_overhead = self.estimate_system_overhead();
        let working_memory = self.memory_budget - system_overhead;
        let memory_per_thread = working_memory / thread_count;
        
        let input_buffer_size = memory_per_thread * 40 / 100;
        let compression_buffer_size = memory_per_thread * 45 / 100;
        let output_buffer_size = memory_per_thread * 15 / 100;
        
        OptimalConfig {
            thread_count,
            codec_threads,
            input_buffer_size,
            compression_buffer_size,
            output_buffer_size,
            compression_level: 3, // Default zstd level
            enable_file_batching: false,
            streaming_mode: false,
            prefetch_factor: 2.0,
            estimated_total_memory: working_memory + system_overhead,
        }
    }

    /// Estimate system overhead (Rust runtime, OS, etc.)
    fn estimate_system_overhead(&self) -> usize {
        // Estimate ~10% of budget or minimum 50MB
        (self.memory_budget / 10).max(50 * 1024 * 1024)
    }

    /// Validate that configuration fits within memory budget ±5%
    pub fn validate_config(&self, config: &OptimalConfig) -> bool {
        let target = self.memory_budget as f64;
        let actual = config.estimated_total_memory as f64;
        let deviation = (actual - target).abs() / target;
        
        deviation <= self.tolerance
    }
}

/// Main AutoTuner that orchestrates bottleneck detection and resource optimization
pub struct AutoTuner {
    detector: BottleneckDetector,
    calculator: ResourceCalculator,
    current_config: Option<OptimalConfig>,
    current_bottleneck: BottleneckType,
    adaptation_counter: usize,
    adaptation_interval: usize, // Retune every N measurements
}

impl AutoTuner {
    pub fn new(memory_budget: MemoryBudget) -> Self {
        Self {
            detector: BottleneckDetector::new(),
            calculator: ResourceCalculator::new(memory_budget),
            current_config: None,
            current_bottleneck: BottleneckType::Balanced,
            adaptation_counter: 0,
            adaptation_interval: 10, // Retune every 10 measurements (5 seconds)
        }
    }

    /// Main tuning method: analyze current state and return optimal configuration
    pub fn tune(&mut self, compression_stats: Option<&CompressionStats>) -> OptimalConfig {
        self.adaptation_counter += 1;
        
        // Collect current performance statistics
        let mut stats = self.detector.collect_stats();
        
        // Update stats with compression-specific information if available
        if let Some(comp_stats) = compression_stats {
            stats.avg_file_size = comp_stats.avg_file_size;
            stats.compression_mbps_per_thread = comp_stats.mbps_per_thread;
        }
        
        // Detect current bottleneck
        let detected_bottleneck = self.detector.detect_bottleneck(&stats);
        
        // Only retune if bottleneck changed or at regular intervals
        let should_retune = detected_bottleneck != self.current_bottleneck 
            || self.adaptation_counter >= self.adaptation_interval
            || self.current_config.is_none();
        
        if should_retune {
            self.current_bottleneck = detected_bottleneck.clone();
            self.adaptation_counter = 0;
            
            let new_config = self.calculator.calculate_optimal_config(detected_bottleneck, &stats);
            
            println!("[AutoTune] Detected bottleneck: {:?}", self.current_bottleneck);
            println!("[AutoTune] New config: threads={}, codec_threads={}, mem_est={}MB", 
                     new_config.thread_count, 
                     new_config.codec_threads,
                     new_config.estimated_total_memory / (1024 * 1024));
            
            self.current_config = Some(new_config.clone());
            new_config
        } else {
            // Return current configuration
            self.current_config.as_ref().unwrap().clone()
        }
    }

    /// Get current bottleneck type
    pub fn current_bottleneck(&self) -> &BottleneckType {
        &self.current_bottleneck
    }
}

/// Compression-specific statistics provided by the compression engine
#[derive(Debug, Clone)]
pub struct CompressionStats {
    /// Average file size being processed
    pub avg_file_size: u64,
    /// Compression throughput per thread in MB/s
    pub mbps_per_thread: f64,
    /// Current compression ratio achieved
    pub compression_ratio: f64,
    /// Number of files processed so far
    pub files_processed: usize,
    /// Total bytes processed so far
    pub bytes_processed: u64,
}
