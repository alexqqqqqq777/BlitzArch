// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::process::{Command, exit};

fn main() {
  // macOS: если программа запущена с аргументами → предполагаем CLI-режим.
  // Переправляем вызов во встроенный бинарь `blitzarch-cli`, расположенный рядом
  // с GUI-исполняемым файлом. Это позволяет использовать одну и ту же .app
  // как для графики, так и для терминала без запуска оконного интерфейса.
  #[cfg(target_os = "macos")]
  if env::args().len() > 1 {
    if let Ok(current_exe) = env::current_exe() {
      if let Some(parent_dir) = current_exe.parent() {
        let cli_exe = parent_dir.join("blitzarch-cli");
        if cli_exe.exists() {
          // Запускаем CLI с теми же аргументами (кроме имени программы)
          match Command::new(&cli_exe)
            .args(env::args().skip(1))
            .status() {
              Ok(status) => {
                // Прокидываем код выхода из дочернего процесса
                match status.code() {
                  Some(code) => exit(code),
                  None => exit(1),
                }
              }
              Err(e) => {
                eprintln!("❌ Не удалось запустить blitzarch-cli: {}", e);
                // падать не будем — откатываемся к GUI
              }
            }
        }
      }
    }
  }

  // GUI (default path)
  app_lib::run();
}
