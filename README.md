# cargo-ai-fdocs

**Version-locked documentation for AI coding assistants.**

Sync README, CHANGELOG, and guides from your Rust dependencies — pinned to the exact versions in your `Cargo.lock` — so Cursor, Copilot, Windsurf, and other AI tools stop hallucinating about APIs that changed three releases ago.

```bash
cargo install cargo-ai-fdocs
cargo ai-fdocs init      # scan Cargo.lock, generate config
cargo ai-fdocs sync      # fetch docs into docs/ai/vendor-docs/rust/
```

## The problem no one talks about honestly

When coding with AI assistants, generated code is often clean and confident — but wrong for your actual dependency versions.

Typical failure pattern:
- Method signatures changed recently
- Extractors/modules were renamed
- Traits moved between crates

Result: you lose time fixing version drift instead of shipping features.

`cargo-ai-fdocs` addresses this directly by providing assistants with docs that match your exact lockfile versions.

## The solution

`ai-fdocs` takes a simple approach:

1. Read `Cargo.lock` to get exact dependency versions.
2. Resolve matching GitHub tags.
3. Download README/CHANGELOG/selected docs.
4. Store them in a predictable local path for AI tools.

No vector DB, no embeddings, no cloud service — just accurate docs in workspace context.

## Why this matters (Documentation Flywheel)

Accurate docs in prompt context produce more accurate generated code. That code propagates into repositories, blog posts, answers, and examples future models train on.

```text
accurate docs in context
  → correct generated code
    → better public code corpus
      → better future model behavior
```

The exact magnitude is unknown, but the direction is clear: reducing API drift in generated code improves trust and outcomes.

## How it works

### 1) Lockfile is truth
`Cargo.lock` defines exact versions. No "latest" guesses.

### 2) Tags are anchors
For each crate version, `ai-fdocs` tries tags like:
- `v1.0.0`
- `1.0.0`
- `<crate>-v1.0.0`
- `<crate>-1.0.0`

If no tag matches, it falls back to default branch and marks this explicitly.

### 3) Freshness is automatic
After `cargo update`, run `cargo ai-fdocs sync`:
- changed versions are re-fetched,
- unchanged versions come from cache,
- removed dependencies are pruned.

`cargo ai-fdocs check` exits with code `1` if docs are stale (CI-friendly).

## Quick start

### 1. Install
```bash
cargo install cargo-ai-fdocs
```

### 2. Generate config from lockfile
```bash
cargo ai-fdocs init
```

This scans `Cargo.lock`, queries crates.io for repository URLs, filters low-level crates, and generates `ai-fdocs.toml`.

### 3. Sync docs
```bash
cargo ai-fdocs sync
```

Default output:

```text
docs/ai/vendor-docs/rust/
├── _INDEX.md
├── axum@0.7.4/
│   ├── _SUMMARY.md
│   ├── .aifd-meta.toml
│   ├── README.md
│   └── CHANGELOG.md
├── serde@1.0.197/
│   ├── _SUMMARY.md
│   ├── .aifd-meta.toml
│   └── README.md
└── tokio@1.36.0/
    ├── _SUMMARY.md
    ├── .aifd-meta.toml
    ├── README.md
    └── CHANGELOG.md
```

### 4. Point your assistant to docs
Example for Cursor (`.cursorrules`):

```text
When working with Rust dependencies, always check docs/ai/vendor-docs/rust/
for version-specific documentation before suggesting API usage. Start with
_INDEX.md for an overview, then read the relevant crate's _SUMMARY.md and
README.md. The documentation is pinned to the exact versions in Cargo.lock.
```

### 5. Keep fresh after updates
```bash
cargo update
cargo ai-fdocs sync
```

## Configuration

Minimal valid config:

```toml
[crates.serde]
sources = [{ type = "github", repo = "serde-rs/serde" }]
```

Extended example (multiple files + notes):

```toml
[settings]
output_dir = "docs/ai/vendor-docs/rust"
max_file_size_kb = 200
prune = true

[crates.serde]
sources = [{ type = "github", repo = "serde-rs/serde" }]
ai_notes = "Use #[derive(Serialize, Deserialize)] for DTOs."

[crates.sqlx]
sources = [{ type = "github", repo = "launchbadge/sqlx" }]
files = ["README.md", "CHANGELOG.md", "docs/migration-guide.md"]
ai_notes = "Use compile-time checked queries with sqlx::query! macro."
```

A ready-to-copy version of this extended example lives in `examples/ai-docs.toml`
(you can copy it to project root as `ai-fdocs.toml`).

### Config reference
- `[settings]` — global behavior
- `[crates.<name>]` — one section per dependency

Fields in `[crates.<name>]`:
- `sources` (required): list of source objects, currently `[{ type = "github", repo = "owner/repo" }]`
- `files` (optional): explicit files to fetch
- `ai_notes` (optional): project-specific guidance injected into index

## Commands

### `cargo ai-fdocs init`
Generate `ai-fdocs.toml` from `Cargo.lock`.

```bash
cargo ai-fdocs init
cargo ai-fdocs init --overwrite
```

### `cargo ai-fdocs sync`
Fetch docs for configured crates.

```bash
cargo ai-fdocs sync
cargo ai-fdocs sync --force
```

### `cargo ai-fdocs status`
Show lockfile versions vs sync state.

### `cargo ai-fdocs check`
Exit `0` when up-to-date, `1` when stale.

```bash
cargo ai-fdocs check || echo "Docs are outdated! Run cargo ai-fdocs sync."
```

## CI integration

### GitHub Actions
```yaml
- name: Check AI docs are fresh
  run: cargo ai-fdocs check
```

### Pre-commit hook
```bash
#!/bin/sh
cargo ai-fdocs check --quiet || {
  echo "AI docs are outdated. Running sync..."
  cargo ai-fdocs sync
  git add docs/ai/vendor-docs/
}
```

## Processing details

- **Tag resolution:** tries common tag patterns, then fallback branch.
- **Filename flattening:** nested paths use `__` (e.g. `docs__guides__overview.md`).
- **CHANGELOG truncation:** keeps current + previous minor context.
- **Size limits:** large files truncated with clear marker.
- **Markdown headers:** adds source URL/ref/fetch date comments.
- **Config hash:** changes in crate config trigger re-fetch automatically.

## GitHub token

Unauthenticated GitHub API: ~60 req/hour. Authenticated: ~5000.

```bash
export GITHUB_TOKEN=ghp_your_token_here
cargo ai-fdocs sync
```

`ai-fdocs` checks `GITHUB_TOKEN` and `GH_TOKEN`.

## Should docs be committed?

Yes, generally. It keeps local, CI, and team context consistent.

Recommended `.gitattributes`:

```gitattributes
docs/ai/vendor-docs/** linguist-generated=true
```

## FAQ

**Q: Why not just paste docs into chat?**  
A: Manual pasting is one-off. `ai-fdocs` is automated, comprehensive, and lockfile-synced.

**Q: Will this bloat the repo?**  
A: Usually modest (README + truncated CHANGELOG per dependency).

**Q: Non-GitHub repos?**  
A: Currently GitHub-first; broader host support is planned.

**Q: Private dependencies?**  
A: Works with a token that has access.

**Q: Workspaces?**  
A: Yes — run from workspace root; root `Cargo.lock` is used.

## Roadmap

### v0.1 (alpha)
- Config parsing
- Cargo.lock resolution
- GitHub tag resolution + fallback
- README/CHANGELOG fetching
- Flattening, headers, truncation
- Caching (`.aifd-meta.toml`)
- Pruning
- `_INDEX.md` generation

### v0.2 (current)
- `init` via crates.io API
- Parallel fetching (8 concurrent)
- `check` command for CI
- `_SUMMARY.md` per crate
- Config hash change detection
- Enhanced status hints

### v0.3 (planned)
- GitLab/Bitbucket support
- docs.rs fallback
- Custom file transforms
- Watch mode (`Cargo.lock`)
- Workspace-aware config inheritance

### v0.4 (planned)
- Semantic chunking for large docs
- Cross-reference linking
- `ai-fdocs explain <crate>`
- Plugin system for custom sources

## License

MIT

## Contributing

Contributions are welcome. Please open an issue first to discuss substantial changes.

If you have data about AI hallucination rates, documentation quality effects, or dependency drift impact, we'd especially love your input.
