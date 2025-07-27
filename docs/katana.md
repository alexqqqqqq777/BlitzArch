# Katana Archive Format

Katana is the default high-performance archive format used by BlitzArch. It was designed from the ground up for maximizing extraction speed, parallel processing, and providing robust security features.

## Key Features

- **Massively Parallel**: Both compression and decompression can utilize all available CPU cores
- **Sharded Architecture**: Data is split into independent shards for efficient parallel processing
- **Random Access**: Quickly extract specific files without reading the entire archive
- **Cross-Platform Compatible**: Works reliably on Windows, macOS, Linux, and other Unix-like systems
- **Strong Security**: Optional AES-256-GCM authenticated encryption
- **Path Safety**: Secure handling of absolute paths and directory traversal attempts

## Cross-Platform Path Handling

Katana implements robust path sanitization to ensure archives can be safely created and extracted across different operating systems:

### Windows/Unix Path Normalization

- **Windows Paths**: Properly handles paths with drive letters (e.g., `C:\Windows\file.txt`), removing drive prefixes and converting backslashes to forward slashes
- **UNC Paths**: Safely processes UNC paths like `\\server\share\file.txt`
- **Unix Paths**: Handles absolute Unix paths (e.g., `/etc/file.txt`)
- **Directory Traversal**: Prevents path traversal attacks using `..` sequences

### Security Features

- **Absolute Path Protection**: When extracting files with absolute paths, Katana automatically sanitizes the paths to prevent overwriting system files
- **Drive Letter Removal**: Windows drive letters (e.g., `C:`) are stripped during extraction
- **Path Component Validation**: Ensures all path components are valid on the target system

### Streaming Encryption

Katana applies **AES-256-GCM** streaming encryption while data is written to the archive.

- Each shard receives its own one-time 12-byte nonce.
- Data is encrypted on-the-fly using a stream cipher, eliminating temporary files and minimizing disk and memory overhead.
- After a shard is written, a 16-byte authentication tag is appended and later validated during extraction.
- The footer stores the CRC32 of the compressed index and an HMAC-SHA-256 (key derived from the password and salt) to guarantee archive integrity and authenticity.

## Technical Implementation

Katana archives consist of:

1. **Header**: Contains format version and global metadata
2. **Shards**: Independent chunks of compressed files
3. **Index**: Metadata for efficient random access
4. **Footer**: Integrity verification and optional encryption metadata

## Archive Creation Options

```bash
# Create a Katana archive (default format)
blitzarch create --output archive.blz ./files

# Create a Katana archive with specific bundle size (in MiB)
blitzarch create --output archive.blz --bundle-size 64 ./files
```

## Extraction Options

```bash
# Extract a Katana archive
blitzarch extract archive.blz

# Extract only specific files
blitzarch extract archive.blz --filter "*.txt"
```

## Performance Characteristics

Katana format excels in scenarios where:

- Multiple CPU cores are available
- Fast extraction is more important than maximum compression ratio
- Archives need to be extracted on different operating systems
- Security and data integrity are important
