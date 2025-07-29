# Changelog

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
