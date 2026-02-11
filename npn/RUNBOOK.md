# npm-ai-fdocs Runbook (B2)

Operational guide for debugging `ai-fdocs` runs in CI and local environments.

## 1) Fast triage checklist

1. Run `ai-fdocs status` to inspect package-level state.
2. Run `ai-fdocs check --format json` for machine-readable drift report.
3. If drift is expected, run `ai-fdocs sync`.
4. If sync fails, classify by error type (401/403, 404, 429, network/parse).

## 2) Debugging common HTTP failures

### 401 / 403 (auth)

Typical causes:
- missing `GITHUB_TOKEN` / `GH_TOKEN`;
- invalid/expired token;
- token without required repository scope (private repos).

Actions:
- verify token is present in CI secrets;
- verify token is exported in job environment;
- retry with a known-good token locally.

### 404 (not found)

Typical causes:
- package repository metadata points to old/renamed repo;
- tag/ref missing for specific version;
- docs files are absent in selected source/layout.

Actions:
- inspect generated `ai-fdocs.toml` and validate `repo/subpath/files`;
- run `ai-fdocs sync` and check fallback behavior (`main/master`);
- if package docs are not required, remove package from config.

### 429 (rate limit)

Typical causes:
- GitHub unauthenticated limit (60 req/hr);
- npm registry throttling.

Actions:
- always set `GITHUB_TOKEN` or `GH_TOKEN` in CI;
- keep retries enabled (already built-in);
- re-run after cooldown for persistent external throttling.

## 3) Token management recommendations

- Prefer short-lived CI tokens where possible.
- Configure one of:
  - `GITHUB_TOKEN` (recommended in GitHub Actions),
  - `GH_TOKEN` (fallback env name).
- Do not print token values in logs.
- Rotate tokens on leakage suspicion.

## 4) `.gitattributes` recommendation

To keep generated docs out of language stats/diff noise:

```gitattributes
docs/ai/vendor-docs/node/** linguist-generated=true
```

## 5) Minimal CI recipe (copy/paste)

```yaml
name: npn-check
on: [pull_request]

jobs:
  npn:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: 'npn/package-lock.json'
      - run: npm install
        working-directory: npn
      - run: npm run build
        working-directory: npn
      - run: node ../../dist/cli.js check --format json
        working-directory: npn/fixtures/check-ok
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## 6) Incident note template

- Timestamp:
- Command:
- Error code / message:
- Affected packages:
- Suspected root cause:
- Mitigation applied:
- Follow-up task:
