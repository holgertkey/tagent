# Changelog

All notable changes to Tagent Text Translator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) with build numbers.

## [0.8.0+039] - 2026-01-06

### Changed
- Speech language now determined by `SourceLanguage` configuration setting
- When `SourceLanguage` is set to "Auto", English is used by default for speech

## [0.8.0+038] - 2026-01-06

### Fixed
- Speech language detection now based on text content (Cyrillic → Russian, Latin → English)

## [0.8.0+037] - 2026-01-06

### Added
- CLI speech command: `-s, --speech` for text-to-speech functionality
- Text-to-speech support using Google Translate TTS API
- Automatic language detection for speech

### Changed
- Help text updated with speech command examples

## [0.8.0+036] - 2025-12-24

### Added
- Automatic version synchronization system in build.rs
- Version now syncs from Cargo.toml to all documentation files automatically
- Detailed version sync reporting in `.debug/` folder

### Changed
- Interactive mode commands now use slash prefix (/) instead of dash (-/--) for better consistency
  - Commands: /h, /help, /?, /c, /config, /v, /version, /clear, /cls, /q, /quit, /exit
- Default alternative hotkey changed from F9 to Alt+Q for better ergonomics
- Configuration moved to AppData folder (`%APPDATA%\Tagent\`)
- Automatic directory creation for config and history files
- Better compliance with Windows application standards
- Cleaner project structure (no config files in program directory)

### Improved
- Documentation updated to reflect automatic version synchronization
- CLAUDE.md now includes detailed version sync mechanism description

### Fixed
- Speech error messages now display correctly with prompt on a new line

## [0.8.0] - 2025-XX-XX

### Added
- Translation history logging with timestamps
- Configurable history file path
- Multi-line format for better readability
- History works in all modes (GUI, CLI, Interactive)

### Changed
- History file now defaults to AppData folder location

## [0.7.0] - 2024-XX-XX

### Added
- Unified interface: GUI hotkeys + Interactive terminal
- Interactive commands with smart recognition
- Simultaneous operation of all translation modes
- Enhanced command-line interface

### Changed
- Application now runs in unified mode by default (GUI + Interactive)
- Improved terminal interaction experience

## [0.6.0 and Earlier] - 2024-XX-XX

### Added
- Basic GUI hotkey functionality (Ctrl+Ctrl double-press)
- CLI translation support
- Dictionary lookup feature
- Configuration management with INI format
- Multi-language support
- Google Translate API integration
- Clipboard operations
- Terminal window management

### Features
- Double-Ctrl hotkey for system-wide translation
- Dictionary definitions with part of speech
- Auto-detection of source language
- Configurable target languages
- Real-time configuration reload

## Version Format

Versions follow the format: `MAJOR.MINOR.PATCH+BUILD`

- **MAJOR**: Incompatible API changes
- **MINOR**: New functionality (backward compatible)
- **PATCH**: Bug fixes (backward compatible)
- **BUILD**: Incremental build number (resets on version change)

Example: `0.8.0+022` = Version 0.8.0, Build 22

[Unreleased]: https://github.com/holgertkey/tagent-win/compare/v0.8.0+022...HEAD
[0.8.0+022]: https://github.com/holgertkey/tagent-win/compare/v0.8.0...v0.8.0+022
[0.8.0]: https://github.com/holgertkey/tagent-win/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/holgertkey/tagent-win/compare/v0.6.0...v0.7.0
[0.6.0 and Earlier]: https://github.com/holgertkey/tagent-win/releases/tag/v0.6.0
