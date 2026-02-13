# AI Fresh Docs - VS Code Extension

Visual Studio Code extension for managing AI Fresh Docs directly in your editor.

## Features

- **📊 Dependency Dashboard**: Visual tree view showing sync status of all dependencies
- **⚡ One-Click Sync**: Sync documentation without leaving VS Code
- **🔄 Auto-Sync**: Automatically sync when lockfiles change (optional)
- **📚 Integrated Viewer**: Click packages to open their documentation
- **🦀 Multi-Language**: Supports both Rust and Node.js projects
- **✅ Status Indicators**: Color-coded icons showing sync status

## Requirements

You need to have `ai-fdocs` binary installed:

**Option 1: NPM (Recommended)**

```bash
npm install -g ai-fdocs
```

**Option 2: Cargo (Rust)**

```bash
cargo install cargo-ai-fdocs
```

## Installation

### From Source (Development)

1. Clone the repository:

```bash
git clone https://github.com/your-org/cargo-ai-fdocs.git
cd cargo-ai-fdocs/vscode
```

1. Install dependencies:

```bash
npm install
```

1. Compile the extension:

```bash
npm run compile
```

1. Press `F5` in VS Code to launch Extension Development Host

### From VSIX (Coming Soon)

```bash
code --install-extension ai-fdocs-0.1.0.vsix
```

## Usage

### Activation

The extension automatically activates when you open a workspace containing:

- `Cargo.toml` (Rust project)
- `package.json` (Node.js project)
- `ai-fdocs.toml` (Configuration file)

### Commands

Access via Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`):

- **AI-Docs: Sync All** - Sync documentation for all dependencies
- **AI-Docs: Force Sync** - Force re-download of all documentation
- **AI-Docs: Initialize** - Create `ai-fdocs.toml` configuration
- **AI-Docs: Prune** - Remove outdated documentation
- **AI-Docs: Check Status** - Check if documentation is up to date
- **AI-Docs: Refresh** - Refresh dependency tree view

### UI Elements

#### Activity Bar

Click the "📚 AI Docs" icon in the activity bar to open the dependency dashboard.

#### Status Bar

Shows current project type:

- `🦀 Rust - AI Docs` for Rust projects
- `📦 Node.js - AI Docs` for Node.js projects

Click to refresh status.

#### Tree View

The dependency tree shows all packages with status indicators:

- ✅ Green checkmark - Synced
- ⚠️ Yellow warning - Outdated
- ❌ Red X - Missing
- 🔧 Wrench - Corrupted

Click any package to open its `_SUMMARY.md` documentation.

## Configuration

Configure the extension via VS Code settings:

```json
{
  "ai-fdocs.binaryPath": "",           // Custom path to ai-fdocs binary
  "ai-fdocs.autoSync": false,          // Auto-sync on lockfile changes
  "ai-fdocs.syncOnSave": false,        // Auto-sync when saving ai-fdocs.toml
  "ai-fdocs.outputDir": "fdocs"        // Output directory for docs
}
```

## Troubleshooting

### Binary Not Found

If you see "ai-fdocs binary not found":

1. Install the binary (see Requirements above)
2. Or configure custom path in settings: `ai-fdocs.binaryPath`
3. Restart VS Code

### Extension Not Activating

Make sure your workspace contains one of:

- `Cargo.toml` for Rust projects
- `package.json` for Node.js projects
- `ai-fdocs.toml` configuration file

## Development

### Building

```bash
npm run compile
```

### Watching

```bash
npm run watch
```

### Testing

```bash
npm test
```

### Packaging

```bash
npm run package
```

## Architecture

The extension is a lightweight wrapper around the `ai-fdocs` CLI:

- **Binary Manager**: Detects and executes `ai-fdocs` binary
- **Project Detector**: Scans workspace for supported projects
- **Tree Provider**: Displays dependencies with status
- **Commands**: Wraps CLI commands with progress notifications

All heavy lifting (fetching, parsing, caching) is done by the Rust/NPM CLI, ensuring consistency.

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for development guidelines.

## License

MIT - See [LICENSE](../LICENSE) for details.

## Links

- [Main Repository](https://github.com/your-org/cargo-ai-fdocs)
- [NPM Package](https://www.npmjs.com/package/ai-fdocs)
- [Cargo Crate](https://crates.io/crates/cargo-ai-fdocs)
- [Documentation](https://github.com/your-org/cargo-ai-fdocs/blob/main/README.md)
