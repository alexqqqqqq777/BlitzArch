//! Stand-alone CLI binary for BlitzArch.
//! Это лёгкая надстройка над общим CLI-раннером из `src/main.rs`,
//! но без автоматического перехода в GUI. Нужен, чтобы внутри
//! MacOS-bundle был отдельный исполняемый файл `blitzarch-cli`,
//! к которому будет проксировать вызовы GUI-launcher при запуске
//! приложения из терминала с аргументами.

fn main() -> std::process::ExitCode {
    if let Err(e) = blitzarch::cli_runner::run_cli_app() {
        if e.downcast_ref::<clap::Error>().is_none() {
            eprintln!("Error: {}", e);
        }
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
