# Changelog

## [0.3.0] - 2025-08-05
### Added
- **Cross-platform release pipeline**: automated macOS ARM GUI, Windows GUI, and Linux CLI builds with attached artifacts and draft release.
- **Version sync**: all components bumped to 0.3.0 and unified.

### Changed
- Suppressed all compiler warnings across benches and tests for cleaner CI logs.
- Updated dependencies (Rust + JS) to latest stable compatible versions via `@tauri-apps/cli 2.6.x`.

### Fixed
- CI failures on macOS due to optional features in `tauri-plugin-dragout`.

### Infrastructure
- Added Release Drafter configuration to generate changelog drafts automatically.
- Introduced conventional commit tagging guidance in `CONTRIBUTING.md`.



All notable changes to this project will be documented in this file.

## [0.1.0] - 2025-07-29

### Added
- **Drag-out**: перетаскивание файлов напрямую из архива в Finder/Explorer с автоматическим извлечением.
- **Batch Extraction**: одновременная распаковка нескольких архивов в их родительские каталоги без доп. диалогов.
- **Paranoid Mode**: опциональная двойная верификация целостности (global BLAKE3-256) — активируется флагом `--paranoid`.
- **Real-time Progress Metrics**: расширенный трекинг скорости, времени и compression ratio в GUI.

### Changed
- **Новая иконка** приложения для GUI и всех дистрибутивов (macOS .icns, Windows .ico, PNG).
- Обновлён интерфейс: drag/drop-зона, прогресс-бар, метрики.

### Fixed
- Исправлена ошибка "Not a Katana archive" в GUI при чтении индекса архива с footer-хешем.
- Исправлена потеря файлов при нулевом `input_buffer_size` (минимум 256 KiB).
- Сняты ограничения на `bundle-size` = 0 (auto-режим).

### Infrastructure
- Добавлен CI workflow GitHub Actions для сборки DMG, MSI, AppImage и ZIP на macOS (ARM/x86), Windows x64 и Linux.
- Все macOS-специфичные зависимости загейчены `cfg(target_os = "macos")` — сборка Windows/Linux без лишних crates.
