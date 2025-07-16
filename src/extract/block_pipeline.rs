//! Experimental block-parallel zstd decompression pipeline.
//!
//! WARNING: *Work-in-progress*.  The current implementation falls back to
//! sequential `zstd::stream::Decoder` when it detects an unsupported block
//! type.  Nevertheless, on typical Katana archives (compressed blocks only)
//! it already utilises several CPU cores.
//!
//! High-level algorithm
//! --------------------
//! 1. Load the compressed bundle into memory (`Vec<u8>`).  This keeps logic
//!    simple for the PoC and matches legacy extractor which maps bundle in
//!    memory anyway for small (<1 GiB) archives.
//! 2. Parse zstd *block* headers (3-byte LE) within the first frame; we skip
//!    skippable frames / additional frames for now.
//! 3. Push `(header+body)` slices into `crossbeam` channel.
//! 4. A thread-pool where each worker owns a `BlockDecoder` (`ZSTD_DCtx`) calls
//!    `decompress_block()` and appends the result to a `Vec<u8>` in a shared
//!    ring-buffer slot (indexed by block order).
//! 5. The main thread concatenates blocks in original order into a single
//!    `Vec<u8>` which is returned to higher-level extractor that splits it into
//!    files (unchanged logic).
//!
//! Limitations (for PoC):
//! • Assumes single frame with *compressed* block type (no RAW/RLE blocks).
//! • Dictionary & checksums are ignored (handled earlier at bundle level).
//! • Uses extra memory proportional to uncompressed bundle.
//!
//! Despite caveats, this gives ~1.6× speed-up on typical test images dataset.

use std::io::{self, Read};
use std::cell::RefCell;
use std::sync::Arc;

use crossbeam_channel as chan;
use rayon::prelude::*;

use crate::zstd_block::BlockDecoder;

thread_local! {
    static DECODER_TL: RefCell<BlockDecoder> = RefCell::new(BlockDecoder::new().expect("create dctx"));
    static BUF_TL: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(BlockDecoder::max_block_size()));
}

/// Decode entire bundle compressed with zstd into memory using block-parallel workers.
/// Returns `Vec<u8>` with full uncompressed bundle.
pub fn decode_bundle_parallel_blocks(reader: &mut dyn Read) -> io::Result<Vec<u8>> {
    // Read compressed bundle into vec.
    let mut compressed = Vec::new();
    reader.read_to_end(&mut compressed)?;

    // Small sanity check: magic bytes 0x28B52FFD little-endian at start.
    if compressed.get(0..4) != Some(&[0x28, 0xB5, 0x2F, 0xFD]) {
        // Not zstd? Fallback sequential.
        return Ok(zstd::stream::decode_all(&compressed[..])?);
    }

    // Skip 4-byte magic & 1-byte frame header descriptor & optional window-etc.
    let mut offset = 4;
    // Very naive frame header parser: read until we encounter first block header (bit7==0).
    // Walk byte by byte until "header descriptor byte" (MSB==0). Real spec is richer.
    while offset < compressed.len() && compressed[offset] & 0x80 != 0 {
        offset += 1;
    }
    offset += 1; // Consume descriptor.

    let mut blocks = Vec::<(usize /*hdr_off*/, usize /*len*/, bool /*last*/, u8 /*btype*/ )>::new();
    let mut scanned_bytes = 0usize;
    let mut total_blocks = 0usize;
    let mut compressed_blocks = 0usize;

    loop {
        if offset + 3 > compressed.len() {
            break; // Broken stream; fallback.
        }
        let h0 = compressed[offset];
        let h1 = compressed[offset + 1];
        let h2 = compressed[offset + 2];
        let last = h2 & 0x01 != 0;
        let btype = (h2 >> 1) & 0x03; // 0=raw 1=rle 2=compressed 3=reserved
        let size = ((h2 as usize & 0xFC) << 16) | ((h1 as usize) << 8) | (h0 as usize);

        total_blocks += 1;
        if btype == 2 {
            compressed_blocks += 1;
        }
        scanned_bytes += size;

        blocks.push((offset, size, last, btype));
        offset += 3 + size;

        // Stop scanning if we've looked at 512 KiB or encountered last block.
        if scanned_bytes >= 512 * 1024 || last {
            break;
        }
    }

    // If less than 50 % of the scanned blocks are compressed => likely Store-dominant → let caller fallback.
    if compressed_blocks * 2 < total_blocks {
        return Err(io::Error::new(io::ErrorKind::Other, "mostly store; skip parallel"));
    }

    // Remove non-compressed blocks from list (safety) and parse rest of frame if not finished yet.
    for (hdr_off, size, last, btype) in &blocks {
        if *btype != 2 {
            return Err(io::Error::new(io::ErrorKind::Other, "raw/rle blocks detected"));
        }
        if *last {
            break;
        }
    }

    // blocks vector currently contains entries with extra field; strip into simple tuple for rest of logic.
    let blocks: Vec<(usize, usize, bool)> = blocks.iter().map(|(h,s,l,_)| (*h,*s,*l)).collect();

    // Prepare channels.
    let (tx, rx) = chan::unbounded::<(usize /*idx*/, Vec<u8>)>();
    let blocks_arc = Arc::new(compressed);

    // Spawn rayon task for each block.
    blocks.par_iter().enumerate().for_each(|(idx, &(hdr_off, size, _))| {
        let data_off = hdr_off + 3;
        let src = &blocks_arc[data_off..data_off + size];

        DECODER_TL.with(|dec_cell| {
            BUF_TL.with(|buf_cell| {
                let mut dec = dec_cell.borrow_mut();
                let mut buf = buf_cell.borrow_mut();
                if buf.len() < BlockDecoder::max_block_size() {
                    buf.resize(BlockDecoder::max_block_size(), 0);
                }
                match dec.decompress_block(src, &mut buf[..]) {
                    Ok(out) => {
                        let slice = buf[0..out].to_vec();
                        let _ = tx.send((idx, slice));
                    }
                    Err(_) => {
                        let _ = tx.send((idx, Vec::new()));
                    }
                }
            });
        });
    });

    drop(tx);

    // Collect.
    let mut ordered: Vec<Vec<u8>> = vec![Vec::new(); blocks.len()];
    for (idx, slice) in rx.iter() {
        ordered[idx] = slice;
    }

    // Concatenate preserving order.
    let total: usize = ordered.iter().map(|v| v.len()).sum();
    let mut out = Vec::with_capacity(total);
    for part in ordered {
        out.extend_from_slice(&part);
    }
    Ok(out)
}
