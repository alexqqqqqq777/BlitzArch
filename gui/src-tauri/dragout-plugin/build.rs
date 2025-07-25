use tauri_plugin::Builder;

// List of commands exposed by the plugin.
const COMMANDS: &[&str] = &["native_drag_out"];

fn main() {
    // Generate autogenerated permissions (allow-*, deny-*) for commands.
    Builder::new(COMMANDS).build();
}
