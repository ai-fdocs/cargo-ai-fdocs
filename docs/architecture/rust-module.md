# Core module: `cargo-ai-fdocs` (Rust)

## 1) Purpose

`cargo-ai-fdocs` syncs dependency documentation for a Rust project using **exact versions from `Cargo.lock`** and stores it locally for AI tooling (Copilot/Cursor/Windsurf, etc.).

Core idea: AI gets context for the real dependency versions used by the project, not stale training-era assumptions.

---

## 2) Architecture and module responsibilities

## CLI orchestrator

- `src/main.rs` — entry point and command routing:
  - `sync` — full synchronization;
  - `status` — status report;
  - `check` — CI freshness check;
  - `init` — `ai-fdocs.toml` generation.

## Configuration

- `src/config.rs`:
  - `ai-fdocs.toml` loading/validation;
  - defaults;
  - backward compatibility with legacy `sources` format.

## Version resolver

- `src/resolver.rs`:
  - resolves crate versions from `Cargo.lock`.

## Network fetcher (GitHub)

- `src/fetcher/github.rs`:
  - resolves git ref (tag/branch) for required version;
  - downloads file contents;
  - applies retries/timeouts;
  - classifies errors (auth/rate-limit/network/not-found).

## Network fetcher (latest-docs)

- `src/fetcher/latest.rs`:
  - resolves latest crate version via crates.io;
  - fetches docs snapshot from docs.rs (`/crate/{name}/{version}`);
  - applies retries/backoff for `429`/`5xx`/network failures;
  - classifies fallback-eligible errors for GitHub degraded mode.

## Storage and cache

- `src/storage.rs`:
  - writes files, `.aifd-meta.toml`, `_SUMMARY.md`;
  - records source provenance in `_SUMMARY.md` (docs.rs vs GitHub fallback in latest-docs);
  - cache checks via config fingerprint;
  - prune of outdated folders.

## Reporting

- `src/status.rs`:
  - builds `Synced / SyncedFallback / Outdated / Missing / Corrupted` statuses;
  - emits machine-readable JSON diagnostics with `mode`, `source_kind`, and `reason_code`.
- `src/index.rs`:
  - generates global `_INDEX.md`.

## Initialization

- `src/init.rs`:
  - reads `Cargo.toml`;
  - calls crates.io metadata;
  - attempts to infer dependency GitHub repositories.

---

## 3) Data flow (where/how it calls external services)

## 3.1 `sync`

### Local input sources

1. `ai-fdocs.toml` (local file).
2. `Cargo.lock` (local file).

### External HTTP calls

For each configured crate:

1. **Tag probing via GitHub API**
   - `GET https://api.github.com/repos/{owner}/{repo}/git/ref/tags/{candidate}`
   - candidates: `v{version}`, `{version}`, `{crate}-v{version}`, `{crate}-{version}`.

2. **Fallback to default branch** (if tags are missing)
   - `GET https://api.github.com/repos/{owner}/{repo}`
   - uses `default_branch`, marks `is_fallback = true`.

3. **File download via raw.githubusercontent.com**
   - `GET https://raw.githubusercontent.com/{owner}/{repo}/{git_ref}/{path}`
   - uses either default file set or explicit `files` from config.

### Local output

- `fdocs/rust/{crate}@{version}/...`
- `.aifd-meta.toml`
- `_SUMMARY.md`
- global `_INDEX.md`

---

## 4) Timing, intervals, retries

## 4.1 Rust fetcher network behavior

- HTTP client timeout: **30 seconds** per request.
- Retries: up to **3 attempts**.
- Base backoff: **500ms**, exponential (`500ms`, `1000ms`).
- Retryable cases:
  - server errors (`5xx`),
  - timeout/connect/request network errors.
- `401` => auth error (no retry).
- `403`/`429` => rate-limit error (no retry).

## 4.2 Concurrency

- `sync` processes crates in parallel.
- Concurrency cap is configurable via `settings.sync_concurrency` (default `8`).

---

## 5) Command behavior

## `cargo ai-fdocs init`

What it does:

1. Checks whether target `ai-fdocs.toml` exists.
2. Reads `Cargo.toml` and collects dependencies (`dependencies` + `workspace.dependencies`).
3. For each dependency, calls crates.io:
   - `GET https://crates.io/api/v1/crates/{crate}`
4. Extracts `owner/repo` from `repository/homepage` when possible.
5. Writes baseline `ai-fdocs.toml`.

Limitations:

- Non-GitHub or non-parsable repositories are skipped.
- If config exists, `--force` is required to overwrite.

## `cargo ai-fdocs sync [--force] [--mode lockfile|latest-docs]`

What it does:

1. Resolves mode (`--mode` overrides `settings.sync_mode`).
2. In `lockfile` mode: loads lockfile versions and runs GitHub file sync.
3. In `latest-docs` mode: resolves latest versions via crates.io, fetches docs.rs artifacts, applies TTL cache checks, and falls back to GitHub when eligible.
4. If `prune = true`, removes outdated folders in lockfile mode.
5. Regenerates global `_INDEX.md`.
6. Prints aggregate stats (synced/cached/skipped/errors + error-type breakdown).

## `cargo ai-fdocs status [--mode lockfile|latest-docs] [--format table|json]`

What it does:

- In `lockfile` mode: compares config + lock versions + stored metadata.
- In `latest-docs` mode: inspects latest artifacts and metadata (without lockfile coupling).
- Prints per-crate status.
- Formats:
  - `table` (default),
  - `json`.

## `cargo ai-fdocs check [--mode lockfile|latest-docs] [--format table|json]`

What it does:

- Runs the same diagnostics as `status` for the resolved mode.
- If issues exist (`Outdated/Missing/Corrupted`) returns non-zero exit code.
- In GitHub Actions, additionally emits `::error` annotations for failing crates.

---

## 6) Configuration and hidden settings

## 6.1 Explicit settings (`[settings]`)

- `output_dir` (default: `fdocs`; Rust output is under `rust` subfolder)
- `max_file_size_kb` (default `200`)
- `prune` (default `true`)
- `sync_concurrency` (default `8`, must be > 0)
- `docs_source` (default `github`)
- `sync_mode` (`lockfile` default, or `latest_docs`/`latest-docs`)
- `latest_ttl_hours` (default `24`; latest-docs cache freshness)
- `docsrs_single_page` (default `true`; docs.rs extraction strategy flag; current runtime supports only `true`)

## 6.2 Per-crate settings (`[crates.<name>]`)

- `repo` — `owner/repo` (preferred format)
- `subpath` — monorepo subpath
- `files` — explicit file list (all listed files are required)
- `ai_notes` — notes embedded into index/summary

## 6.3 Hidden/non-obvious settings

1. **`GITHUB_TOKEN` / `GH_TOKEN`**
   - without token, GitHub API limits are lower;
   - with token, limits are higher.

2. **Fallback git-ref mode**
   - if no version tag is found, default branch is used;
   - this is non-fatal but explicitly marked as fallback.

3. **Cache invalidation via fingerprint**
   - important crate-config changes trigger resync.

4. **Header injection into markdown/html**
   - saved docs include a service header with origin/ref/path/date.

5. **CHANGELOG post-processing**
   - changelog content is additionally truncated around relevant version context.

6. **Large-file truncation**
   - file content is truncated to `max_file_size_kb` and tagged with `[TRUNCATED ...]`.

---

## 7) Usage patterns

## 7.1 Local developer workflow

1. `cargo ai-fdocs init`
2. adjust `ai-fdocs.toml`
3. `cargo ai-fdocs sync`
4. optionally add `fdocs/** linguist-generated=true` to `.gitattributes`

## 7.2 CI quality gate

- run `cargo ai-fdocs check` in PR/merge pipeline;
- pipeline fails when dependency docs are stale or missing.

## 7.3 Scheduled refresh

- run `cargo ai-fdocs sync` on schedule;
- commit refreshed docs artifacts.

---

## 8) Failure/degraded-mode behavior

The tool follows best-effort behavior:

- one-crate failures do not abort the entire `sync`;
- existing cache is preserved except where explicit prune rules apply;
- issues are surfaced by `status/check`.

This keeps development pipelines stable even during temporary network/source outages.
