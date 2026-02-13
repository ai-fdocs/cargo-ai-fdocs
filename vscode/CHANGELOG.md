# Changelog

All notable changes to the "ai-fdocs" extension will be documented in this file.

## [0.1.0] - 2026-02-13

### Added

- Initial release of AI Fresh Docs VS Code extension
- Dependency tree view with status indicators
- Commands: Sync, Force Sync, Initialize, Prune, Check Status, Refresh
- Binary detection for both NPM and Rust versions
- Project detection for Rust and Node.js workspaces
- Status bar integration showing project type
- Auto-sync on lockfile changes (optional)
- Click-to-open documentation from tree view
- Progress notifications for long-running operations
- Configuration settings for binary path, auto-sync, and output directory

### Features

- **Visual Dashboard**: Tree view showing all dependencies with color-coded status
- **One-Click Sync**: Sync documentation without leaving VS Code
- **Multi-Language Support**: Works with both Rust (Cargo) and Node.js (NPM) projects
- **Smart Detection**: Automatically detects project type and binary location
- **File Watchers**: Optional auto-sync when lockfiles or config changes

### Technical

- TypeScript implementation
- Wraps existing `ai-fdocs` CLI (Rust or NPM version)
- Parses JSON output from CLI for status information
- Minimal dependencies for fast activation
