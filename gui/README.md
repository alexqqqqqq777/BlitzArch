# BlitzArch GUI

Modern cross-platform desktop application for BlitzArch archiver built with Rust + Tauri + React.

## Features

- **Drag & Drop Interface**: Simply drag files/folders to create archives
- **Real-time Progress**: Live progress bars with speed metrics and ETA
- **Batch Operations**: Process multiple archives simultaneously
- **Archive Browser**: View and extract individual files from archives
- **Settings Panel**: Configure compression levels, memory limits, and security options
- **Integrity Verification**: Visual feedback for BLAKE3 hash verification
- **Cross-Platform**: Native performance on Windows, macOS, and Linux

## Development

### Prerequisites
- Node.js 18+
- Rust 1.75+
- Tauri CLI: `cargo install tauri-cli`

### Running in Development

```bash
# Install dependencies
npm install

# Start development server
npm run tauri dev
```

### Building for Production

```bash
# Build the application
npm run tauri build
```

This will create platform-specific installers in `src-tauri/target/release/bundle/`

## Architecture

- **Frontend**: React + Vite for modern web UI
- **Backend**: Rust with Tauri for native system integration
- **IPC**: Tauri commands for secure frontend-backend communication
- **Progress Tracking**: Real-time updates via Tauri events
- **File Operations**: Native file system access with drag-out support

## Support

For issues and feature requests, please visit: https://github.com/alexqqqqqq777/BlitzArch/issues