# ai-fdocs (NPM) v0.2

Node.js/TypeScript version of `ai-fdocs` with core feature parity for Rust v0.2:

- `init` from `package.json` (direct dependencies) + npm registry;
- `sync` with parallel downloads (`settings.sync_concurrency`, default `8`, must be > 0);
- `check` for CI (exit code 0/1);
- `_SUMMARY.md` in each package folder;
- `config_hash` for automatic cache invalidation;
- improved `status` with hints.

## Docs source mode

ADR-0001 fixed the default source strategy to **`npm_tarball`**.

### Default behavior

If `docs_source` is not set, `npm-ai-fdocs` uses:

```toml
[settings]
docs_source = "npm_tarball"
```

This mode is recommended for stable CI behavior and release-aligned docs collection.

### Explicit source examples

Use npm tarball explicitly:

```toml
[settings]
docs_source = "npm_tarball"
```

Use GitHub explicitly:

```toml
[settings]
docs_source = "github"
```

### Fallback and degraded behavior

- `sync` is **best-effort**: one package source failure should not abort the whole run;
- already downloaded docs are preserved in cache when a source is temporarily unavailable;
- `status` / `check` report drift and errors for affected packages;
- in `github` mode, branch fallback (`main`/`master`) remains enabled where applicable.

For backward compatibility, legacy `experimental_npm_tarball` is still accepted and treated as `docs_source = "npm_tarball"`.

## Safety and degraded-source behavior

`npm-ai-fdocs` is designed to operate safely in degraded mode when docs sources
are temporarily unavailable (GitHub/npm registry):

- must not break application code or project source files;
- must not fail the entire run because of one problematic package (best-effort);
- should preserve previously downloaded cache;
- should report errors clearly in `status/check` and CI.

Result: network issues reduce docs freshness, but not platform stability.

## Quick start

```bash
npm install
npm run build
node dist/cli.js --help
```

From repository root you can also use helper scripts:

```bash
./scripts/fdocs-sync.sh
./scripts/fdocs-clean.sh
```

## Commands

- `ai-fdocs init [--overwrite]`
- `ai-fdocs sync [--force] [--report-format text|json]`
  - `--report-format json` prints JSON-only output (no extra log lines).
- `ai-fdocs status`
- `ai-fdocs check [--format text|json]`

## Stable CLI contract and SemVer policy

`npm-ai-fdocs` treats CLI behavior as a public contract for automation and CI.

Stable contract includes:

- command names and top-level flags (`init`, `sync`, `status`, `check`);
- machine-readable formats intended for CI (`check --format json`, `sync --report-format json`);
- exit code semantics for `check` (0/1);
- metadata compatibility guarantees documented for releases.

SemVer policy:

- **Patch (`x.y.Z`)**: bug fixes and non-breaking internal/docs updates.
- **Minor (`x.Y.z`)**: backward-compatible features and new optional flags.
- **Major (`X.y.z`)**: breaking CLI/output/metadata changes or compatibility drops.

Compatibility matrix and release process are documented in:

- [`COMPATIBILITY.md`](./COMPATIBILITY.md)
- [`RELEASING.md`](./RELEASING.md)

## Roadmap

Detailed roadmap: [`ROADMAP.md`](./ROADMAP.md).

ADR по выбору default source: [`docs/adr/0001-docs-source-strategy.md`](./docs/adr/0001-docs-source-strategy.md).


## Runbook

Operational troubleshooting and CI guidance: [`RUNBOOK.md`](./RUNBOOK.md).

## Migration notes

Migration guidance for existing configs (`docs_source`, legacy `experimental_npm_tarball`): [`MIGRATION.md`](./MIGRATION.md).
