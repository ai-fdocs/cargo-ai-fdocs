# ai-fdocs (NPM) v0.2

Node.js/TypeScript version of `ai-fdocs` with core feature parity for Rust v0.2:

- `init` from `package.json` (direct dependencies) + npm registry;
- `sync` with parallel downloads (`MAX_CONCURRENT=8`);
- `check` for CI (exit code 0/1);
- `_SUMMARY.md` in each package folder;
- `config_hash` for automatic cache invalidation;
- improved `status` with hints.

## Experimental docs source mode

By default, documentation is fetched from a package's GitHub repository.

You can enable an experimental mode that fetches docs from the npm tarball:

```toml
[settings]
experimental_npm_tarball = true
```

> ⚠️ This is an experimental mode and may behave differently for non-standard packages.

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

## Commands

- `ai-fdocs init [--overwrite]`
- `ai-fdocs sync [--force]`
- `ai-fdocs status`
- `ai-fdocs check [--format text|json]`

## Roadmap

Detailed roadmap: [`ROADMAP.md`](./ROADMAP.md).


## Runbook

Operational troubleshooting and CI guidance: [`RUNBOOK.md`](./RUNBOOK.md).
