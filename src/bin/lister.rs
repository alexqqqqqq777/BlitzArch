use std::path::Path;
use std::fs::File;
use blitzarch::extract::list_files;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_path = Path::new("test.blz");
    println!("Listing files in {:?}:", archive_path);
    let file = File::open(archive_path)?;
    list_files(file)?;
    Ok(())
}
