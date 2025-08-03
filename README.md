# BlitzArch

[![Crates.io](https://img.shields.io/crates/v/blitzarch.svg)](https://crates.io/crates/blitzarch) 
[![Docs.rs](https://docs.rs/blitzarch/badge.svg)](https://docs.rs/blitzarch)
[![License](https://img.shields.io/badge/license-GPLv3%20or%20Commercial-blue.svg)](./LICENSE)

**BlitzArch (`blz`)** is a high-performance file archiver designed for speed, efficiency, and modern hardware. Written in Rust, it leverages lock-free data structures, multi-threaded processing, and advanced I/O techniques to deliver exceptional performance for both creating and extracting archives.

## Philosophy

- **Performance First**: Every design decision is made with performance as the primary goal. This includes using the blazingly fast Zstandard compressor, maximizing CPU core utilization, and optimizing I/O patterns.
- **Modern and Simple**: A straightforward command-line interface without a maze of legacy options. We focus on a few powerful commands that work well.
- **Secure by Default**: Optional, but robust, end-to-end encryption using modern, authenticated ciphers (AES-256-GCM).

## Key Features

- **High-Speed Compression/Decompression**: Powered by `zstd` for a great balance of speed and compression ratio.
- **Massively Parallel**: Utilizes all available CPU cores for both compression and decompression to minimize processing time.
- **Optimized I/O**: Employs techniques like memory-mapped files and sharded workers to reduce I/O bottlenecks.
- **Memory-Aware Compression**: New `--memory-budget` flag lets you cap RAM usage; Katana dynamically scales codec threads to stay within the limit.
- **The `.blz` (Katana) Format**: A custom, highly parallelizable archive format designed from the ground up for maximum extraction speed.
- **Cross-Platform Compatible**: Robust path handling that works consistently across Windows, macOS, and Linux with secure sanitization of absolute paths.
- **Strong Encryption**: On-the-fly **streaming** AES-256-GCM encryption (no temp files) keeps your data secure.
- **Modern GUI**: Cross-platform desktop application with real-time progress tracking, drag & drop support, and batch operations.
- **AutoTune Technology**: Intelligent resource management automatically optimizes threads, memory usage, and compression parameters based on system capabilities.
- **Enterprise-Ready**: Built-in integrity verification, compliance support, and comprehensive logging for audit trails.

## System Requirements

### Minimum Requirements
- **OS**: Windows 10+, macOS 10.15+, or Linux (glibc 2.31+)
- **CPU**: 2+ cores (4+ cores recommended for optimal performance)
- **RAM**: 2GB minimum (8GB+ recommended for large archives)
- **Storage**: 100MB for installation + temporary space for archive operations

### Recommended for Enterprise Use
- **CPU**: 8+ cores with modern instruction sets (AVX2/AVX-512)
- **RAM**: 16GB+ for processing large datasets (>100GB archives)
- **Storage**: NVMe SSD for optimal I/O performance
- **Network**: High-bandwidth connection for distributed workflows

## Download (pre-built binaries)

Grab the latest release from the **[GitHub Releases page](https://github.com/alexqqqqqq777/BlitzArch/releases)**:

| OS | File | Quick install |
|----|------|---------------|
| Windows | `BlitzArch-<version>.msi` | download & double-click |
| macOS (x86_64) | `blitzarch-<version>-macos-x86_64.zip` | `curl -L https://github.com/alexqqqqqq777/BlitzArch/releases/latest/download/blitzarch-$(uname -m).zip -o blitzarch.zip && unzip blitzarch.zip && chmod +x blitzarch` |
| Linux (x86_64, glibc 2.31+) | `blitzarch-<version>-linux-x86_64.tar.gz` | `curl -L https://github.com/alexqqqqqq777/BlitzArch/releases/latest/download/blitzarch-linux-x86_64.tar.gz | tar -xz && chmod +x blitzarch` |

Once downloaded, run `blitzarch --help` to see the commands.

---

## Installation

### From Crates.io (Recommended)

Once published, you will be able to install BlitzArch using `cargo`:

```bash
cargo install blitzarch
```

### From Source

Ensure you have the Rust toolchain installed.

```bash
# Clone the repository
git clone https://github.com/alexqqqqqq777/BlitzArch.git
cd BlitzArch

# Build the release binary
cargo build --release

# The executable will be at target/release/blitzarch
./target/release/blitzarch --help
```

## Usage

### `create`: Create an Archive

```bash
# Create an archive (Katana format is **on by default**, level 3, auto threads)
blitzarch create --output my_archive.blz ./source_folder

# High-ratio mode (level 7)
blitzarch create --output my_archive.blz --level 7 ./source_folder

# Best-ratio mode (level 12, slow & RAM-heavy)
blitzarch create --output my_archive.blz --level 12 ./source_folder

# Encrypt an archive with a password
blitzarch create --output secret.blz --password "your-password" ./private_docs
```

### `extract`: Extract an Archive

```bash
# Extract to the current directory
blitzarch extract my_archive.blz

# Extract to a specific directory
blitzarch extract my_archive.blz --output ./restored_files

# Extract an encrypted archive (will prompt for password if not provided)
blitzarch extract secret.blz --password "your-password"
```

### Modern GUI Application

BlitzArch includes a cross-platform desktop GUI built with Tauri and React:

```bash
# Download GUI from releases or build from source
cd gui
npm install
npm run tauri dev  # Development mode
npm run tauri build  # Production build
```

**GUI Features:**
- **Drag & Drop Interface**: Simply drag files/folders to create archives
- **Real-time Progress**: Live progress bars with speed metrics and ETA
- **Batch Operations**: Process multiple archives simultaneously
- **Archive Browser**: View and extract individual files from archives
- **Settings Panel**: Configure compression levels, memory limits, and security options
- **Integrity Verification**: Visual feedback for BLAKE3 hash verification
- **Cross-platform**: Native performance on Windows, macOS, and Linux

### `list`: List Archive Contents

```bash
# List the contents of an archive
blitzarch list my_archive.blz
```

## Advanced Options

BlitzArch exposes several power-user flags beyond the common `create / extract / list` workflow.

| Flag | Purpose |
|------|---------|
| `--adaptive` | Skips compression for blocks detected as incompressible, saving CPU time on large binary blobs. Katana does this automatically; this flag mostly benefits legacy tar-like workflows via the library API.|
| `--memory-budget N` | Limit RAM used by Katana compression. Accepts: absolute size in **MiB** (e.g. `500`), or percentage of system RAM when suffixed with `%` (e.g. `50%`). `0` or omitted = unlimited. Katana auto-adjusts codec threads to fit the budget. |
| `--use-lzma2` / `--lz-level N` | Switch the compressor from Zstandard (default) to multi-threaded LZMA2. Helpful when maximum ratio is critical and extra CPU time/RAM is acceptable. |
| `--bundle-size N` | Target bundle size in **MiB** for Katana archives. Larger bundles improve ratio; smaller favour parallelism. Default: **32 MiB**. |
| `--codec-threads N` | Threads _inside_ each compressor (0 = auto). |
| `--threads N` | Total worker threads for archive creation (0 = auto, default: all CPU cores). |
| `--strip-components N` | During extraction, remove N leading path components from each file (same as `tar --strip-components`). Useful to avoid absolute paths or deep directory nesting. |
| `--skip-check` | **⚠️ UNSAFE**: Skip final BLAKE3-256 integrity verification after archive creation. Only use for benchmarks or when integrity is not critical. **Security risk!** |
| `--no-adaptive` | Disable adaptive compression (force compression of all data, even incompressible). By default, BlitzArch skips compression for files that don't benefit from it. |
| `--progress` | Show real-time progress bar during `create` or `extract` operations. Displays speed, ETA, and completion percentage. |

> **Deprecated / hidden flags**: `--sharded`, `--seekable`, `--preprocess` – these experimental or legacy options have been removed from the public CLI.

## AutoTune Technology

BlitzArch features **intelligent resource management** that automatically optimizes performance based on your system capabilities:

### Adaptive Configuration
- **Bottleneck Detection**: Automatically detects whether your system is Memory-Bound or I/O-Bound
- **Thread Optimization**: Dynamically adjusts worker threads and codec threads for optimal throughput
- **Memory Management**: Intelligent buffer sizing based on available RAM and workload characteristics
- **Compression Strategy**: Selects optimal compression levels and algorithms for your hardware

### Performance Results
Based on extensive benchmarking with real-world datasets:

| Configuration | Throughput | Improvement |
|---------------|------------|-------------|
| Baseline (fixed params) | 150 MB/s | - |
| AutoTune enabled | 275+ MB/s | **+83%** |
| AutoTune + Adaptive buffers | 401 MB/s | **+167%** |

*Results measured on 8-core system with 70,007 files (848MB dataset)*

## Integrity & Compliance

BlitzArch provides **comprehensive integrity verification by default**. After archive creation, a dual verification process ensures data integrity:

* **Per-shard validation**: File count and BLAKE3 checksum verification for each compressed shard
* **Global integrity check**: Full-archive BLAKE3-256 hash verification (paranoid mode)

This **secure-by-default** approach ensures archives meet regulatory requirements for immutable storage (WORM/tamper-evident). Integrity verification can **only** be disabled with explicit `--skip-check` flag (CLI) or "Skip integrity check (unsafe)" checkbox in GUI. We strongly recommend keeping verification enabled except for micro-benchmarks.

### Regulatory Compliance Support

BlitzArch helps organizations comply with major data retention and integrity standards:

* **SEC 17a-4(f)** (United States Securities)
* **FINRA 4511 / CFTC 1.31** (Financial Records Retention) 
* **Sarbanes-Oxley §404** (Internal Controls)
* **GDPR Art 5(1)(f)** (Integrity and Confidentiality)
* **ISO/IEC 27001 Annex A** (Information Security Controls A.8.2 & A.12.7)
* **NIST Cybersecurity Framework** (Data Integrity functions)

### Enterprise Security Features

* **BLAKE3-256 cryptographic hashing** for tamper detection
* **AES-256-GCM authenticated encryption** with streaming support
* **Argon2 key derivation** for password-based encryption
* **Path sanitization** prevents directory traversal attacks
* **Memory-safe Rust implementation** eliminates buffer overflows

## Performance Benchmarks

### Compression Performance
Benchmarked on various systems with real-world datasets:

#### Standard Workstation (8-core, 16GB RAM)
```
Dataset: 70,007 files, 848MB total
┌─────────────────┬──────────────┬──────────────┬─────────────┐
│ Configuration   │ Throughput   │ Ratio        │ Time        │
├─────────────────┼──────────────┼──────────────┼─────────────┤
│ BlitzArch (L3)  │ 275 MB/s    │ 4.34x        │ 3.1s        │
│ BlitzArch (L7)  │ 195 MB/s    │ 5.12x        │ 4.3s        │
│ 7-Zip (Max)     │ 45 MB/s     │ 4.89x        │ 18.8s       │
│ WinRAR (Best)   │ 38 MB/s     │ 4.45x        │ 22.3s       │
└─────────────────┴──────────────┴──────────────┴─────────────┘
```

#### Enterprise Server (32-core, 128GB RAM)
```
Dataset: Large mixed content, 50GB total
┌─────────────────┬──────────────┬──────────────┬─────────────┐
│ Configuration   │ Throughput   │ Ratio        │ Time        │
├─────────────────┼──────────────┼──────────────┼─────────────┤
│ BlitzArch (L3)  │ 850+ MB/s    │ 4.2x         │ 62s         │
│ tar + zstd      │ 420 MB/s     │ 3.8x         │ 125s        │
│ tar + gzip      │ 180 MB/s     │ 3.1x         │ 285s        │
└─────────────────┴──────────────┴──────────────┴─────────────┘
```

### Key Performance Advantages
- **Parallel Processing**: Utilizes all CPU cores effectively
- **SIMD Optimizations**: Leverages modern CPU instruction sets
- **Zero-Copy Operations**: Minimizes memory allocations and copies
- **Intelligent I/O**: Optimized read/write patterns reduce disk bottlenecks

## Enterprise Support & Troubleshooting

### Logging and Monitoring
```bash
# Enable detailed logging
export RUST_LOG=debug
blitzarch create --output archive.blz /path/to/data

# Performance profiling
blitzarch create --output archive.blz --verbose /path/to/data
```

### Common Issues & Solutions

**High Memory Usage**
```bash
# Limit memory consumption
blitzarch create --memory-budget 50% --output archive.blz /data
```

**Slow Performance on Network Storage**
```bash
# Optimize for I/O-bound scenarios
blitzarch create --bundle-size 64 --threads 4 --output archive.blz /network/data
```

**Large File Handling**
```bash
# Process very large archives efficiently
blitzarch create --bundle-size 128 --codec-threads 0 --output huge.blz /massive/dataset
```

### API Documentation
For programmatic integration, see the [Rust API documentation](https://docs.rs/blitzarch) and [GUI integration examples](./gui/README.md).

---

## License

BlitzArch is **dual-licensed**:

1. **GPL v3** – free for personal, educational and open-source use. You may modify and redistribute under the same license.
2. **Commercial License** – required for proprietary, internal or SaaS use without copyleft obligations. See [`LICENSE-commercial.txt`](./LICENSE-commercial.txt) or contact <aleksandr.krayz@gmail.com>.

By using BlitzArch you agree to the terms of either license.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for details on how to get started.
