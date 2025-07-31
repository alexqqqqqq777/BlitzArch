use std::io::Write;
use std::fs::File;

// The library crate is assumed to be named `blitzarch`. Adjust if different.
use blitzarch::katana::read_katana_footer;

#[test]
fn test_read_katana_footer_basic() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_path = std::env::temp_dir().join("blitzarch_footer_test.blz");

    // Craft a minimal fake archive: [dummy data][compressed index][footer]
    let mut file = File::create(&tmp_path)?;
    let dummy = vec![0u8; 100];
    file.write_all(&dummy)?;

    let idx_comp_size: u64 = 10;
    let idx_json_size: u64 = 20;

    // write dummy "compressed index" of size idx_comp_size
    file.write_all(&vec![0u8; idx_comp_size as usize])?;

    // now footer: comp_size (LE), json_size (LE), magic
    file.write_all(&idx_comp_size.to_le_bytes())?;
    file.write_all(&idx_json_size.to_le_bytes())?;
    file.write_all(b"KATIDX01")?;
    file.flush()?;
    drop(file);

    let mut f = File::open(&tmp_path)?;
    let (comp_size, comp_offset, json_size) = read_katana_footer(&mut f)?;

    assert_eq!(comp_size, idx_comp_size);
    assert_eq!(json_size, idx_json_size);
    assert_eq!(comp_offset, f.metadata()?.len() - 24 - comp_size);

    std::fs::remove_file(&tmp_path).ok();
    Ok(())
}
