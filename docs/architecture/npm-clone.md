# NPM clone: `ai-fdocs` in `npn/` (Node.js/TypeScript)

## 1) Purpose

The NPM clone mirrors core Rust-module behavior for the JavaScript/TypeScript ecosystem:

- resolves dependencies from lockfiles;
- fetches documentation for exact package versions;
- stores docs in local `vendor-docs/node`;
- provides `init/sync/status/check` for local workflows and CI.

---

## 2) Architecture and modules

## CLI

- `npn/src/cli.ts` — command wiring and centralized error handling.

## Commands

- `npn/src/commands/init.ts`
- `npn/src/commands/sync.ts`
- `npn/src/commands/status.ts`
- `npn/src/commands/check.ts`

## Config

- `npn/src/config.ts` — `ai-fdocs.toml` loading.

## Lockfile resolver

- `npn/src/resolver.ts` — supports:
  - `package-lock.json`,
  - `pnpm-lock.yaml`,
  - `yarn.lock`.

## External source integrations

- `npn/src/fetcher.ts`:
  - GitHub API / raw.githubusercontent.com;
  - optional docs fetch from npm tarballs.
- `npn/src/registry.ts`:
  - npm registry metadata;
  - GitHub repo/subpath extraction.

## Storage

- `npn/src/storage.ts`:
  - file/meta writes;
  - cache checks;
  - prune logic.

---

## 3) External calls (where/how it calls)

## 3.1 `init`

For each candidate from `package.json` (dependencies + devDependencies):

1. `GET https://registry.npmjs.org/{package}`
2. Extract GitHub repo from `repository` field.
3. If `repository.directory` exists, store as `subpath`.

Behavior details:

- on HTTP `429` metadata response, waits `2 seconds` and retries once;
- packages without repo / non-GitHub repo are skipped.

## 3.2 `sync` in GitHub mode (default)

For each configured package:

1. Resolve version from lockfile.
2. Probe refs:
   - `GET https://api.github.com/repos/{repo}/git/ref/heads/{ref}`
   - `GET https://api.github.com/repos/{repo}/git/ref/tags/{ref}`
   - candidates: `v{version}`, `{version}`;
   - fallback: `main`, then `master`.
3. Fetch repository tree:
   - `GET https://api.github.com/repos/{repo}/git/trees/{ref}?recursive=1`
4. Download files:
   - `GET https://raw.githubusercontent.com/{repo}/{ref}/{path}`

## 3.3 `sync` in npm tarball mode

If `settings.docs_source = "npm_tarball"` (default):

1. Resolve package version payload:
   - `GET https://registry.npmjs.org/{package}/{version}`
2. Read `dist.tarball` URL.
3. Download and unpack tarball locally.
4. Select docs files by the same rules (or explicit `files`).

---

## 4) Timing, intervals, and limits

## 4.1 Concurrency

- `sync` uses `p-limit`.
- Concurrency cap comes from `settings.sync_concurrency` (default `8`).

## 4.2 Delays/throttling

- `init`:
  - sleeps `50ms` every 10 packages to reduce request spikes;
  - prints progress every 30 packages.
- npm registry metadata:
  - on `429`, sleeps `2000ms` and retries once.

## 4.3 File selection limits

- default mode picks docs/readme/changelog/license markdown patterns from tree;
- both modes cap output to first **40** files.

---

## 5) Command behavior

## `ai-fdocs init [--overwrite]`

What it does:

1. Checks if `ai-fdocs.toml` already exists.
2. Reads `package.json` (dependencies + devDependencies).
3. Filters known low-signal packages (for example `typescript`, `eslint`, `@types/*`, etc.).
4. Queries npm registry metadata.
5. Generates `ai-fdocs.toml` with defaults:
   - `output_dir = "fdocs/node"`
   - `prune = true`
   - `max_file_size_kb = 512`
   - `sync_concurrency = 8`
   - `docs_source = "npm_tarball"`

## `ai-fdocs sync [--force]`

What it does:

1. Loads config and lockfile.
2. If `prune=true`, removes outdated package folders.
3. Per package:
   - if missing from lockfile => `skipped`;
   - if valid cache and no `--force` => `cached`;
   - otherwise fetches docs (GitHub or tarball mode);
   - on success saves files + metadata;
   - generates package `_SUMMARY.md`.
4. Generates global index.
5. Prints aggregate stats: synced/cached/skipped/errors.

## `ai-fdocs status`

- Checks folder and `.aifd-meta.toml` presence;
- validates config hash;
- reports statuses:
  - `✅ Synced`
  - `⚠️ Synced (fallback: main/master)`
  - `⚠️ Config changed (resync needed)`
  - `❌ Missing`
  - `❌ Not in lockfile`

Additional output:

- `.gitattributes` recommendation;
- GitHub token status (set/not set).

## `ai-fdocs check`

- Verifies all configured package docs are present and up to date.
- On issues, prints list and exits with code `1`.
- On success, exits with code `0`.

---

## 6) Settings and hidden parameters

## 6.1 Explicit TOML settings

`[settings]`:

- `output_dir` (default `fdocs/node`)
- `prune` (default `true`)
- `max_file_size_kb` (default `512`, must be > 0)
- `sync_concurrency` (default `8`, must be > 0)
- `docs_source` (`npm_tarball` by default, can be `github`)

`[packages.<name>]`:

- `repo` (required for GitHub-based sync)
- `subpath` (monorepo subpath)
- `files` (explicit files)
- `ai_notes`

## 6.2 Hidden/non-obvious parameters

1. **`GITHUB_TOKEN` / `GH_TOKEN`**
   - used for GitHub API requests.

2. **Cache invalidation by `config_hash`**
   - stored in `.aifd-meta.toml`;
   - changes in repo/subpath/files/notes trigger resync.

3. **Header injection**
   - `.md/.html/.htm` files include a source metadata header.

4. **File truncation**
   - content is truncated to `max_file_size_kb` and tagged with `[TRUNCATED ...]`.

5. **Changelog processing**
   - changelog files get additional truncation/post-processing.

---

## 7) Usage scenarios

## Local workflow

1. `npm install`
2. `npm run build`
3. `node dist/cli.js init`
4. adjust `ai-fdocs.toml`
5. `node dist/cli.js sync`

## CI gate

- run `node dist/cli.js check`;
- job fails when docs are stale/missing.

## Tarball-first workflow

- keep default `docs_source = "npm_tarball"`;
- optionally switch to `docs_source = "github"` for repo-first retrieval.

---

## 8) Degraded mode

The NPM clone is also best-effort:

- one package failure should not abort full sync;
- existing cache is preserved;
- issues are clearly reported in status/check.

Result: documentation freshness may degrade during outages, but platform stability remains intact.
