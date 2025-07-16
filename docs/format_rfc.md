> **⚠️ Legacy Draft**
>
> The following document describes an experimental `MicroFusion` (`.mfus`) container that was **never shipped**.
> It is kept for historical reference only. The production BlitzArch format is **Katana** (`.blz`) and differs significantly.

# MicroFusion (aatrnnbdye) File Format RFC

This document specifies the byte-level layout of the `.mfus` container file.

## 1. Overview

- **Magic Bytes**: `MFUSv01` (8 bytes)
- **Primary Header**: 1KB JSON object, zstd compressed.
- **Bloom Filter**: For fast file existence checks.
- **Primary Index**: Maps file paths to data bundle offsets.
- **Data Bundles**: zstd seekable frames.
- **Footer**: Contains a replica of the header and telemetry data.

## 2. Byte Layout

| Offset       | Size (bytes) | Description                |
|--------------|--------------|----------------------------|
| `0`          | `8`          | Magic Bytes `[M,F,U,S,v,0,1,\0]` |
| `8`          | `1016`       | Primary Header (JSON+zstd) |
| `1024`       | `var`        | Bloom Filter data          |
| `...`        | `var`        | Primary Index data         |
| `...`        | `var`        | Data Bundles...            |

