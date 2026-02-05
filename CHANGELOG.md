# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0-alpha.1] - 2026-02-05

### Added
- Multi-source management: manage multiple feed/sitemap sources in database
- CLI commands: `add`, `remove`, and `list` subcommands for source management
- Legacy configuration migration for existing users

### Changed
- Refactored submission functions to use direct parameters instead of config
- Simplified command-line argument handling in CLI struct
- Enhanced README with detailed source management instructions

### Fixed
- Updated macOS platform description in README
- Removed redundant warning about first run requirement for unattended mode

## [0.1.0-alpha.1] - 2026-01-18

### Added
- Initial alpha release
- RSS/Atom/JSON feed and sitemap monitoring
- IndexNow API integration for search engine submissions
- SQLite database for URL tracking
- Interactive configuration and dry-run modes
- Support for multiple platforms (Linux, macOS, Windows)
- Bulk submission with up to 10,000 URLs per batch
- `unattended` subcommand for automated submissions without confirmation

### Changed
- Updated README with unattended mode documentation and warnings
- Improved .gitignore with comprehensive Rust project patterns

### Features
- Multi-format support: RSS, Atom, JSON Feed, XML Sitemap
- Smart modification detection using dates
- First-run safety with confirmation prompts
- Color-coded terminal output
- Comprehensive error handling and validation