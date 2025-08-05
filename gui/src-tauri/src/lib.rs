// Commands module with all Tauri command functions
mod commands;
pub use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    #[cfg(target_os = "macos")]
    .plugin(tauri_plugin_dragout::init())
    
    .plugin(tauri_plugin_dialog::init())
    .invoke_handler(tauri::generate_handler![
        create_archive,
        create_archive_async,
        get_parent_directory,
        get_downloads_path,
        extract_archive,
        extract_archive_async,
        list_archive,
        list_archive_async,
        drag_out_extract,
        cleanup_drag_out_temp,
        create_link_file,
        delete_file,
        get_system_metrics,
        native_drag_out_global
    ])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
