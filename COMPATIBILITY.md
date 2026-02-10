# Compatibility and Support Policy

This document defines stability guarantees for `cargo-ai-fdocs` toward v1.0.

## Scope

`cargo-ai-fdocs` is a CLI tool consumed by:
- humans via terminal output and help text,
- CI systems via exit codes and JSON output (`status/check --format json`),
- AI workflows via generated docs layout and metadata.

## Versioning

We follow [Semantic Versioning](https://semver.org/) once `v1.0.0` is released.

- **Patch (`x.y.Z`)**
  - bug fixes,
  - performance/observability improvements,
  - no breaking changes in documented contracts.
- **Minor (`x.Y.z`)**
  - backward-compatible features,
  - new optional config fields,
  - additive JSON fields (never removing/renaming existing ones).
- **Major (`X.y.z`)**
  - breaking changes in CLI, config schema, JSON contract, or on-disk format.

## Stability guarantees

### 1) CLI

After `v1.0.0`:
- existing subcommands and documented flags are stable,
- removing/renaming flags or changing semantics is a major change,
- help text wording may improve in patch/minor releases if behavior is unchanged.

### 2) Exit codes

For `cargo ai-fdocs check`:
- `0` means all configured crates are in a synced state,
- non-zero means at least one crate is not synced or command execution failed.

These semantics are considered stable after `v1.0.0`.

### 3) JSON output contract (`status/check --format json`)

Stable fields:
- top-level: `summary`, `statuses`,
- `summary`: `total`, `synced`, `missing`, `outdated`, `corrupted`,
- `statuses[]`: `crate_name`, `lock_version`, `docs_version`, `status`, `reason`.

Rules:
- existing fields are not removed/renamed outside a major release,
- new fields may be added in minor releases,
- status enum values are append-only in minor releases; removals/renames require major release.

### 4) Config file compatibility (`ai-fdocs.toml`)

- existing documented keys stay supported through all `1.x` releases,
- new keys are additive and optional in minor releases,
- key removal/semantic incompatibility requires a major release.

### 5) Cache and metadata compatibility

- `.aifd-meta.toml` schema is versioned via `schema_version`,
- older supported schemas may be migrated on read,
- unknown future schema versions are treated as incompatible and ignored safely.

## Platform support

Targeted support for stable releases:
- Linux
- macOS
- Windows

CI smoke checks are executed across all three platforms to validate build/test basics.

## Deprecation policy

- Deprecations are announced in docs/changelog before removal.
- Deprecated behavior remains available for at least one minor release before removal in a future major version.

## Non-goals of compatibility guarantees

The following are intentionally not strict API contracts:
- exact formatting/alignment of table output,
- ordering of non-contract informational log lines,
- transient warning wording when semantics are unchanged.
