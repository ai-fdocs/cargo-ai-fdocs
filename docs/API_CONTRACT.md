# API Contract: crates.io + docs.rs latest-docs pipeline

This document defines the **implementation contract** for the `latest_docs` sync mode.

## 1) Goals

- Resolve latest stable crate version from crates.io.
- Fetch documentation content from docs.rs for that exact version.
- Fallback to GitHub docs files when docs.rs content is unavailable.
- Keep behavior deterministic, observable, and CI-friendly.

---

## 2) Endpoints and responsibilities

## 2.1 crates.io (version authority)

### Endpoint
- `GET https://crates.io/api/v1/crates/{crate_name}`

### Required extracted fields
- `crate.max_stable_version` (preferred)
- fallback: `crate.max_version` if `max_stable_version` is empty

### Validation rules
- Version must be non-empty valid semver-like string.
- Prerelease versions are skipped unless explicitly enabled in future config.

### Error mapping
- `401/403` -> `Auth` / `RateLimit` category
- `404` -> `NotFound` (crate does not exist)
- `429` -> `RateLimit`
- `5xx` -> retryable `Network/Upstream`
- malformed JSON or missing expected fields -> `Other` (contract violation)

---

## 2.2 docs.rs (content authority)

### Primary endpoint (MVP)
- `GET https://docs.rs/crate/{crate_name}/{version}`

### Optional endpoint (future enrichment)
- `GET https://docs.rs/{crate_name}/{version}/{crate_name}/` (crate root rustdoc page)

### Required output artifact (MVP)
- `API.md` (single-page normalized snapshot)

### Extraction rules
- Keep only main article/rustdoc body section.
- Remove script/style noise not useful for AI context.
- Preserve code blocks, signatures, and headings.
- Rewrite relative links to absolute `https://docs.rs/...` links.

### Error mapping
- `404` -> docs for version not built yet (fallback-eligible)
- `429` -> `RateLimit`
- `5xx` -> retryable
- parsing failure (HTML shape changed) -> `Other` but fallback-eligible

---

## 2.3 GitHub fallback (already implemented)

Used only when docs.rs fetch fails and crate has GitHub source configured.

### Existing behavior to preserve
- ref resolution by tag candidates then default branch fallback
- file fetch with retries/backoff
- optional/required file semantics

(See current GitHub fetcher behavior in codebase.)

---

## 3) Sync algorithm (latest_docs mode)

For each configured crate:

1. Resolve `latest_version` via crates.io.
2. Check cache/meta:
   - if TTL valid and cached version/source are fresh -> use cache.
3. Fetch docs snapshot from docs.rs for `{crate}@{latest_version}`.
4. If docs.rs fails with fallback-eligible errors -> try GitHub file fallback.
5. Save crate folder:
   - `.aifd-meta.toml`
   - `_SUMMARY.md`
   - `API.md` (docs.rs success) and/or fallback files
6. Record sync outcome and source kind.

---

## 4) Caching and freshness contract

## 4.1 Config knobs
- `sync_mode = "latest_docs"`
- `latest_ttl_hours = 24` (default)
- `docsrs_single_page = true` (default)

## 4.2 Meta fields (required)
- `schema_version`
- `version`
- `sync_mode` (`lockfile` | `latest_docs`)
- `source_kind` (`docsrs` | `github_fallback` | `mixed`)
- `upstream_latest_version`
- `upstream_checked_at`
- `ttl_expires_at`
- existing: `git_ref`, `is_fallback`, `fetched_at`

## 4.3 TTL policy
- if `now < ttl_expires_at` -> skip remote calls (unless `--force`)
- if expired -> revalidate latest version via crates.io
- if latest changed -> hard refresh
- if latest unchanged -> soft refresh only if previous source was fallback and retry window reached

---

## 5) Fallback policy

## 5.1 Fallback-eligible errors from docs.rs
- 404 not built yet
- 429 rate-limited
- transient network/5xx
- parse failure due to temporary layout drift

## 5.2 Non-fallback errors
- invalid crate name from config
- permanent config validation errors

## 5.3 Observability requirements
- `_SUMMARY.md` must state whether docs came from docs.rs or fallback.
- `.aifd-meta.toml` must contain source kind and upstream check timestamps.
- `status/check --format json` must expose mode/source fields.

---

## 6) Status/check semantics

## lockfile mode
- unchanged current behavior: compare `docs_version` with `Cargo.lock` version.

## latest_docs mode
- compare stored `upstream_latest_version` vs current crates.io latest.
- `Synced` if equal and TTL valid.
- `Outdated` if upstream changed or cache stale and refresh failed.
- `SyncedFallback` if up-to-date but source is fallback.
- `Corrupted` if meta invalid/missing required fields.

---

## 7) Retry/backoff requirements

For crates.io and docs.rs HTTP calls:
- attempts: 3
- backoff: 500ms, 1000ms, 2000ms
- retry on timeouts/connectivity/5xx
- no retry on 4xx except 429

---

## 8) Test plan requirements

## Unit tests
- crates.io response parsing and version selection.
- docs.rs single-page extraction normalization.
- meta serialization with new fields.
- status/check mode-aware logic.

## Integration tests
- happy path (crates.io + docs.rs success).
- docs.rs 404 -> GitHub fallback.
- crates.io 429 / network timeout.
- TTL valid path skips network.
- force refresh bypasses TTL.

## Regression tests
- existing lockfile mode behavior unchanged.

---

## 9) Storage layout (target)

```text
docs/fdocs/
├── _INDEX.md
├── serde@1.0.228/
│   ├── .aifd-meta.toml
│   ├── _SUMMARY.md
│   └── API.md
└── tokio@1.48.0/
    ├── .aifd-meta.toml
    ├── _SUMMARY.md
    └── API.md
```

---

## 10) Non-goals for MVP

- Full recursive mirror of entire rustdoc asset tree.
- Private registry support.
- Multi-language ecosystem support (outside Rust).


---

## 11) Virtual end-to-end walkthrough (A -> Z)

This is a deterministic dry-run that must pass in implementation and tests.

1. CLI starts `sync --mode latest-docs`.
2. Config is validated:
   - `sync_mode=latest_docs`
   - crate entries exist
   - source config is coherent.
3. For each crate job (bounded concurrency):
   - resolve upstream latest version via crates.io;
   - validate returned version string.
4. Read local meta/cache for `crate@latest`:
   - if meta missing -> cache miss;
   - if meta invalid schema -> corrupted -> force refresh;
   - if TTL valid and same upstream version -> cache hit.
5. If refresh needed:
   - fetch docs page from docs.rs;
   - parse/normalize to single-page artifact `API.md`;
   - if docs.rs fails with fallback-eligible reason -> run GitHub fallback.
6. Persist artifacts atomically:
   - write temp dir;
   - write docs files + `_SUMMARY.md` + `.aifd-meta.toml`;
   - rename temp dir to final `crate@version`.
7. Update global index (`_INDEX.md`) after all crate jobs.
8. Emit sync summary with source counters:
   - docsrs success
   - github fallback
   - cached
   - failed.
9. `status/check` mode-aware validation:
   - latest_docs compares against crates.io latest,
   - lockfile compares against Cargo.lock.
10. CI consumes JSON report with explicit reason codes.

---

## 12) Reliability hardening checklist ("tank mode")

Mandatory guards before enabling by default:

- [ ] Atomic writes for crate directories (no partial final state).
- [ ] Strict timeout budget per request and per crate job.
- [ ] Retry with exponential backoff and capped attempts.
- [ ] Circuit-breaker behavior for repeated upstream failures.
- [ ] Idempotent reruns: same input -> same output tree.
- [ ] Deterministic sorting in index and status outputs.
- [ ] Structured reason codes for every non-success branch.
- [ ] Schema-versioned meta with forward-compatibility checks.
- [ ] Fallback provenance in both summary and meta.
- [ ] Golden tests for parser normalization output.
- [ ] Integration tests with mocked crates.io/docs.rs/GitHub outages.
- [ ] Regression test suite for existing lockfile mode.

---

## 13) Reason code matrix (required for status/check JSON)

- `latest_ok_docsrs`
- `latest_ok_fallback`
- `latest_cache_hit_ttl`
- `latest_outdated_upstream_changed`
- `latest_outdated_refresh_failed`
- `latest_corrupted_meta`
- `latest_missing_no_artifacts`
- `lockfile_ok`
- `lockfile_outdated_version_mismatch`
- `lockfile_missing`
- `lockfile_corrupted_meta`

All non-success reason codes must map to actionable remediation text.
