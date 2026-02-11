# Compatibility matrix

This document defines the **supported runtime matrix** for `npm-ai-fdocs`.

## Node.js (LTS)

`npm-ai-fdocs` supports active Node.js LTS lines:

- Node.js 20.x (LTS)
- Node.js 22.x (LTS)

Notes:

- EOL Node.js versions are not supported.
- New LTS lines may be added in minor releases.
- Dropping an LTS line is only allowed in a major release.

## Operating systems

| OS family | Architecture | Support level |
| --- | --- | --- |
| Linux | x64 / arm64 | Supported |
| macOS | Apple Silicon / Intel | Supported |
| Windows | x64 | Supported |

Support notes:

- CI coverage targets Linux first.
- macOS/Windows are supported as long as Node.js runtime and filesystem behavior remain compatible.
- Platform-specific regressions are fixed on a best-effort basis before patch release when reproducible.

## Compatibility policy

- The compatibility matrix is part of the public release contract.
- Any change to this matrix must be reflected in release notes.
- Breaking compatibility changes require a SemVer major version bump.
