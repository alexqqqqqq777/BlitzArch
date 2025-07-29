fn main() {
    // --- Windows resource embedding (manifest, icon) ---
    // --- Windows resource embedding (manifest, icon) ---
    // Выполняем только если это *основной* бинарник, иначе при линковке
    // конечного `app.exe` (Tauri) получаются дубликаты VERSION ресурсов.
    #[cfg(target_os = "windows")]
    if std::env::var("CARGO_BIN_NAME").is_ok() {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("windows_app.manifest");
        // Optional: uncomment when icon available
        res.set_icon("assets/blitzarch.ico");
        res.set("FileDescription", "BlitzArch Archiver");
        res.set("ProductName", "BlitzArch");
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.compile().expect("failed to compile Windows resources");
    }

    // If the crate is built with --features simd_optim we want to instruct the compiler
    // to use native CPU features for **this** crate and all downstream crates.  A simple
    // portable way is to inject an environment variable that Cargo passes to rustc for
    // every subsequent build script step: `RUSTFLAGS`.
    //
    // This is not perfect (users can still override RUSTFLAGS manually) but gives an
    // easy opt-in switch without requiring developers to remember long commands.
    if std::env::var("CARGO_FEATURE_SIMD_OPTIM").is_ok() {
        // Equivalent of setting: RUSTFLAGS="-C target-cpu=native"
        println!("cargo:rustc-env=RUSTFLAGS=-Ctarget-cpu=native");
        // Emit cfg flag so the Rust code can `#[cfg(simd_optim)]` guard blocks.
        println!("cargo:rustc-cfg=simd_optim");
    }
}
