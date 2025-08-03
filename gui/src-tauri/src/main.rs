// BlitzArch unified binary: acts as CLI when invoked with subcommands; otherwise starts the GUI.
use std::process::ExitCode;

fn try_cli_mode() -> Option<ExitCode> {
    let mut args = std::env::args();
    let _exe = args.next();
    let Some(first) = args.next() else { return None };

    match first.as_str() {
        "create" | "extract" | "list" | "help" | "-h" | "--help" => {
            if let Err(e) = blitzarch::cli_runner::run_cli_app() {
                eprintln!("Error: {e}");
                return Some(ExitCode::FAILURE);
            }
            Some(ExitCode::SUCCESS)
        }
        _ => None,
    }
}

fn main() -> ExitCode {
    if let Some(code) = try_cli_mode() {
        return code;
    }
    app_lib::run();
    ExitCode::SUCCESS
}
