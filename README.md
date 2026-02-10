# cargo-ai-fdocs

**Version-locked documentation for AI coding assistants.**

`cargo-ai-fdocs` helps close the knowledge gap between AI training data and the
exact dependency versions used in your Rust project.

It syncs README/CHANGELOG/guides from GitHub repositories for versions pinned in
`Cargo.lock`, then stores them locally under `docs/ai/vendor-docs/rust/` so
Cursor, Copilot, Windsurf, and other assistants can use up-to-date context.

## Why this exists

In practice, many AI coding failures happen not because the model cannot reason,
but because it references outdated APIs. This frequently causes trust loss:
compilation fails, developers stop relying on the assistant, and productivity
falls back to manual lookup.

We treat this as an engineering hygiene problem:
- lockfile version is the source of truth;
- docs are fetched for that exact version (or fallback branch with warning);
- local docs are refreshed after dependency updates.

## Current alpha scope (this repository)

Implemented now:
- parse project config (`ai-docs.toml`);
- resolve crate versions from `Cargo.lock`;
- fetch docs from GitHub (including custom file lists);
- cache per crate/version with metadata;
- prune outdated crate folders;
- generate global index (`_INDEX.md`);
- show status of synced docs.

Current commands:

```bash
cargo ai-docs sync
cargo ai-docs sync --force
cargo ai-docs status
```

> Note: the package name is `cargo-ai-fdocs`, while the current alpha command
> flow in this branch uses `cargo ai-docs ...`.

## Quick start

1. Install

```bash
cargo install cargo-ai-fdocs
```

2. Create `ai-docs.toml`

```toml
[settings]
output_dir = "docs/ai/vendor-docs/rust"
max_file_size_kb = 200
prune = true

[crates.axum]
sources = [{ type = "github", repo = "tokio-rs/axum" }]
ai_notes = "Prefer extractor-based handlers and Router-first composition."

[crates.sqlx]
sources = [{ type = "github", repo = "launchbadge/sqlx" }]
files = ["README.md", "CHANGELOG.md", "docs/migration-guide.md"]
ai_notes = "Prefer compile-time checked queries with sqlx::query!"
```

3. Sync docs

```bash
cargo ai-docs sync
```

By default files are stored in:

```text
docs/ai/vendor-docs/rust/
├── _INDEX.md
├── axum@<version>/
│   ├── .aifd-meta.toml
│   ├── README.md
│   └── CHANGELOG.md
└── sqlx@<version>/
    ├── .aifd-meta.toml
    ├── README.md
    └── docs__migration-guide.md
```

## How it works

1. Read exact crate versions from `Cargo.lock`.
2. Resolve a matching Git ref for each configured crate.
3. Download default or explicit file list from GitHub.
4. Truncate oversized files and process CHANGELOG content.
5. Save docs in versioned folders and write crate metadata.
6. Regenerate `_INDEX.md` for AI navigation.

## Configuration reference

`ai-docs.toml` supports:

- `[settings]`
  - `output_dir` (default: `docs/ai/vendor-docs/rust`)
  - `max_file_size_kb` (default: `200`)
  - `prune` (default: `true`)

- `[crates.<name>]`
  - `sources` (required in current alpha format)
  - `files` (optional explicit file list)
  - `ai_notes` (optional hints included in index)

## Practical AI integration

For Cursor-like tools, point instructions to:
- `docs/ai/vendor-docs/rust/_INDEX.md` first,
- then the crate folder matching the current lockfile version.

This reduces stale API suggestions and makes generated code more consistent
with your project’s real dependency graph.

## Roadmap (high level)

Planned next steps include `init`, `check`, and parallel fetching, but these are
not part of the currently released alpha behavior in this branch.

## License

MIT
