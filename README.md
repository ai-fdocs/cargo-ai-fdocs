# cargo-ai-fdocs

**AI Fresh Docs (Rust CLI): version-locked documentation for AI coding assistants.**

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
- parse project config (`ai-fdocs.toml`);
- resolve crate versions from `Cargo.lock`;
- fetch docs from GitHub (including custom file lists);
- cache per crate/version with metadata and config fingerprint invalidation;
- prune outdated crate folders;
- generate global index (`_INDEX.md`);
- show status of synced docs;
- continue sync when one crate/file fails (best-effort), reporting errors in итоговой статистике.
- run crate sync in parallel for faster lockfile processing.

Current commands:

```bash
cargo ai-fdocs sync
cargo ai-fdocs sync --force
cargo ai-fdocs status
cargo ai-fdocs status --format json
cargo ai-fdocs check
cargo ai-fdocs check --format json
cargo ai-fdocs init
```

> Note: the package name is `cargo-ai-fdocs`, while the current alpha command
> flow in this branch uses `cargo ai-fdocs ...`.

## Quick start

1. Install

```bash
cargo install cargo-ai-fdocs
```

2. Create `ai-fdocs.toml`

```toml
[settings]
output_dir = "docs/ai/vendor-docs/rust"
max_file_size_kb = 200
prune = true
sync_concurrency = 8

[crates.axum]
repo = "tokio-rs/axum"
ai_notes = "Prefer extractor-based handlers and Router-first composition."

[crates.sqlx]
repo = "launchbadge/sqlx"
files = ["README.md", "CHANGELOG.md", "docs/migration-guide.md"]
ai_notes = "Prefer compile-time checked queries with sqlx::query!"
```

3. Sync docs

```bash
cargo ai-fdocs sync
```

By default files are stored in:

```text
docs/ai/vendor-docs/rust/
├── _INDEX.md
├── axum@<version>/
│   ├── .aifd-meta.toml
│   ├── _SUMMARY.md
│   ├── README.md
│   └── CHANGELOG.md
└── sqlx@<version>/
    ├── .aifd-meta.toml
    ├── _SUMMARY.md
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

`ai-fdocs.toml` supports:

- `[settings]`
  - `output_dir` (default: `docs/ai/vendor-docs/rust`)
  - `max_file_size_kb` (default: `200`)
  - `prune` (default: `true`)
  - `sync_concurrency` (default: `8`)

- `[crates.<name>]`
  - `repo` (recommended, `owner/repo`)
  - `subpath` (optional monorepo prefix for default files)
  - `files` (optional explicit file list)
  - `ai_notes` (optional hints included in index)

Legacy `sources = [{ type = "github", repo = "..." }]` is still accepted for
backward compatibility, but new configs should use `repo`.

## Practical AI integration

In CI (`cargo ai-fdocs check`), failures include per-crate reasons; in GitHub Actions they are additionally emitted as `::error` annotations.

### CI recipes (GitHub Actions)

#### 1) `check` gate (PR/merge safety)

```yaml
name: ai-fdocs-check
on:
  pull_request:
  push:
    branches: [main]

jobs:
  check-docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Check ai-fdocs status (JSON)
        run: cargo ai-fdocs check --format json
```

#### 2) `sync` updater (scheduled + manual)

```yaml
name: ai-fdocs-sync
on:
  workflow_dispatch:
  schedule:
    - cron: "0 6 * * 1" # weekly, Mondays 06:00 UTC

jobs:
  sync-docs:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Sync docs
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: cargo ai-fdocs sync

      - name: Commit updated docs
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git add docs/ai/vendor-docs/rust ai-fdocs.toml
          git diff --cached --quiet || git commit -m "chore: refresh ai-fdocs"

      - name: Push changes
        run: git push
```

#### 3) `cache`-aware variant (explicit Cargo cache keys)

```yaml
name: ai-fdocs-check-cache
on:
  pull_request:

jobs:
  check-docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry + target
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: Run check
        run: cargo ai-fdocs check --format json
```

### JSON output contract (`status/check --format json`)

Top-level object:
- `summary`: counters for current run
  - `total`, `synced`, `missing`, `outdated`, `corrupted`
- `statuses`: per-crate entries
  - `crate_name`, `lock_version`, `docs_version`, `status`, `reason`

`status` enum values:
- `Synced`
- `SyncedFallback`
- `Outdated`
- `Missing`
- `Corrupted`


For Cursor-like tools, point instructions to:
- `docs/ai/vendor-docs/rust/_INDEX.md` first,
- then the crate folder matching the current lockfile version.

This reduces stale API suggestions and makes generated code more consistent
with your project’s real dependency graph.

## Roadmap to stable release

### 1) Reliability hardening (near-term)
- [x] Improve retry/backoff behavior for GitHub API and raw-content downloads.
- [x] Add clearer error classification (auth/rate-limit/not-found/network) in `sync` summary.
- [ ] Expand integration tests for lockfile parsing and fetch fallback scenarios.

### 2) CI and team workflows
- [x] Stabilize `cargo ai-fdocs check` exit codes for predictable CI gating.
- [x] Add machine-readable check output mode (JSON) for CI/report tooling.
- [x] Provide ready-to-copy GitHub Actions recipes in docs.

### 3) Output and cache stability
- [x] `.aifd-meta.toml` carries explicit schema versioning (`schema_version = 1`) as a stable baseline.
- [x] Legacy cache metadata without schema version is auto-migrated on read; newer unknown schema versions are treated as incompatible.
- [ ] Improve index ergonomics (`_INDEX.md`) for large dependency graphs.

### 4) Release readiness (v1.0)
- [x] Finalize CLI UX and help text consistency across all subcommands.
- [x] Complete cross-platform smoke checks (Linux/macOS/Windows).
- [x] Publish a compatibility/support policy and semantic-versioning guarantees.

### 5) Tooling technical debt (next refactor candidate)
- [ ] `cargo clippy` may still report `too_many_arguments` for
  `storage::save_crate_files`; non-blocking for release, but a near-term refactor target.

## License

MIT
