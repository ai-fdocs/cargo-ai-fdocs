# Development Setup Instructions

Since NPM is not available in your environment, here are the manual steps to set up and test the extension:

## Prerequisites

1. **Install Node.js and NPM**
   - Download from: <https://nodejs.org/>
   - Recommended version: Node.js 20.x LTS
   - Verify installation: `node --version` and `npm --version`

2. **Install ai-fdocs binary**
   - NPM version: `npm install -g ai-fdocs`
   - OR Rust version: `cargo install cargo-ai-fdocs`

## Setup Steps

1. **Navigate to extension directory**

   ```bash
   cd c:\Users\user\cargo-ai-fdocs\vscode
   ```

2. **Install dependencies**

   ```bash
   npm install
   ```

3. **Compile TypeScript**

   ```bash
   npm run compile
   ```

4. **Run in development mode**
   - Open the `vscode` folder in VS Code
   - Press `F5` to launch Extension Development Host
   - A new VS Code window will open with the extension loaded

## Testing

### Manual Testing

1. **Open a test project**
   - Open the root `cargo-ai-fdocs` folder (Rust project)
   - OR open the `npn` folder (Node.js project)

2. **Verify extension activation**
   - Check status bar for "🦀 Rust - AI Docs" or "📦 Node.js - AI Docs"
   - Open Activity Bar and look for "📚 AI Docs" icon

3. **Test commands**
   - Open Command Palette (`Ctrl+Shift+P`)
   - Try "AI-Docs: Sync All"
   - Try "AI-Docs: Check Status"
   - Try "AI-Docs: Refresh"

4. **Test tree view**
   - Click "📚 AI Docs" in Activity Bar
   - Verify dependencies are listed
   - Click a package to open its documentation

### Automated Testing

```bash
npm test
```

## Building VSIX Package

To create a distributable package:

```bash
npm install -g @vscode/vsce
npm run package
```

This creates `ai-fdocs-0.1.0.vsix` which can be installed via:

```bash
code --install-extension ai-fdocs-0.1.0.vsix
```

## Troubleshooting

### TypeScript Errors

If you see TypeScript errors:

```bash
npm run compile
```

### Extension Not Loading

1. Check Output panel → "AI Fresh Docs"
2. Verify binary is installed: `ai-fdocs --version`
3. Check workspace has `Cargo.toml` or `package.json`

### Binary Not Found

Configure custom path in settings:

```json
{
  "ai-fdocs.binaryPath": "C:\\path\\to\\ai-fdocs.exe"
}
```

## Next Steps

After testing the basic functionality:

1. Implement CodeLens provider (Phase 6)
2. Implement Hover provider (Phase 6)
3. Add unit tests (Phase 8)
4. Polish UI and error messages (Phase 9)
5. Prepare for marketplace publishing (Phase 10)
