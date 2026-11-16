# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog], and this project adheres to [Semantic Versioning].

## [Unreleased]

## [0.2.0] - 2026-11-16

### Added

- Main, Options and Help menus with keyboard and mouse navigation.
- Clickable panel-border controls with highlighted hotkeys.
- Live settings, saved preferences and config reload without disconnecting servers.
- All 49 bundled btop themes, offline theme selection and live previews.
- Custom panel layouts and presets, including startup selection with `--preset`.
- Block and ASCII graphs, 256-color and TTY modes, and Vim navigation.
- CPU graph sources and temperature units, memory graphs/meters, pool filters
  and aggregate disk I/O views.
- Network interface selection, graph scaling, bitrate units and resettable
  session estimates.
- App sorting, display units, details, follow and pause controls.
- HTTP server connections alongside HTTPS.
- Installers for macOS, Linux and Windows, and updates from Main → Update.

### Changed

- The default background is black and the dashboard title is purple.
- App sorting uses Left/Right; pool selection uses brackets. Use p/P for presets
  and m/Esc for Main.

### Fixed

- Server fields now support moving the cursor and editing text in place.

## [0.1.0] - 2026-11-15

- Initial release

[Unreleased]: https://aaronmallen.dev/aaron/tntop/compare/0.2.0...HEAD
[0.2.0]: https://aaronmallen.dev/aaron/tntop/compare/0.1.0...0.2.0
[0.1.0]: https://aaronmallen.dev/aaron/tntop/releases/tag/0.1.0

[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html
