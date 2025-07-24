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

Katana применяет потоковое шифрование **AES-256-GCM** непосредственно во время записи данных.

- Каждому шардy соответствует собственный одноразовый nonce (12 байт).
- Данные шифруются «на лету» через стрим-шифратор без создания временных файлов, поэтому нагрузка на диск и память минимальна.
- После записи шарда добавляется тег аутентификации (16 байт), который проверяется при извлечении.
- В футере хранится CRC32 сжатого индекса и HMAC-SHA-256 (ключ выводится из пароля и соли), что гарантирует целостность и подлинность архива.

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
