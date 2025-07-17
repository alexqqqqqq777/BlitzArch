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
- **The `.blz` (Katana) Format**: A custom, highly parallelizable archive format designed from the ground up for maximum extraction speed.
- **Cross-Platform Compatible**: Robust path handling that works consistently across Windows, macOS, and Linux with secure sanitization of absolute paths.
- **Strong Encryption**: AES-256-GCM authenticated encryption to keep your data secure.

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
git clone https://github.com/your-username/blitzarch.git
cd blitzarch

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

# Disable Katana and fall back to a tar-like container with Zstandard
blitzarch create --no-katana --output legacy.tar.zst ./source_folder

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

### `list`: List Archive Contents

```bash
# List the contents of an archive
blitzarch list my_archive.blz
```

## Advanced Options

BlitzArch exposes several power-user flags beyond the common `create / extract / list` workflow.

| Flag | Purpose |
|------|---------|
| `--no-katana` | Switch off Katana and use a simple tar-like container compressed with Zstandard. |
| `--adaptive` | Automatically stores incompressible chunks instead of compressing them, saving CPU time on large binary blobs. |
| `--use-lzma2` / `--lz-level N` | Switch the compressor from Zstandard (default) to multi-threaded LZMA2. Helpful when maximum ratio is critical and extra CPU time/RAM is acceptable. |
| `--bundle-size N` | Target bundle size in **MiB** for Katana archives. Larger bundles improve ratio; smaller favour parallelism. Default: **32 MiB**. |
| `--codec-threads N` | Threads _inside_ each compressor (0 = auto). |

> **Deprecated / hidden flags**: `--sharded`, `--seekable`, `--preprocess`, `--katana` – these experimental or legacy options are no longer maintained and will be removed in a future release.

## License

BlitzArch is **dual-licensed**:

1. **GPL v3** – free for personal, educational and open-source use. You may modify and redistribute under the same license.
2. **Commercial License** – required for proprietary, internal or SaaS use without copyleft obligations. See [`LICENSE-commercial.txt`](./LICENSE-commercial.txt) or contact <aleksandr.krayz@gmail.com>.

By using BlitzArch you agree to the terms of either license.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for details on how to get started.
