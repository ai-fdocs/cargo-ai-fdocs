# Releasing

This guide defines the minimal release process for `npm-ai-fdocs`.

## Release checklist

1. **Prepare branch**
   - Ensure `main` is green (`npm test`, build passes).
   - Ensure no uncommitted changes.
2. **Review contract changes**
   - Confirm CLI interface and output formats are unchanged, or explicitly documented as breaking.
   - Confirm metadata format changes are backward compatible (or planned for major release).
3. **Update changelog**
   - Add release section with user-visible changes.
   - Group entries by `Added / Changed / Fixed / Docs / Internal`.
4. **Version bump**
   - Apply SemVer rules from `README.md`.
   - Update version in `package.json`.
5. **Validation**
   - `npm ci`
   - `npm run build`
   - `npm test`
   - Smoke-check CLI (`node dist/cli.js --help`, `check --format json`).
6. **Tag and publish**
   - Create annotated git tag: `vX.Y.Z`.
   - Publish package to npm.
7. **Post-release**
   - Verify package install from npm.
   - Announce release with changelog link and compatibility notes.

## Changelog process

- Keep changelog entries user-focused (impact first, implementation details second).
- Every merged PR should contain a changelog note or explicit `no-changelog` rationale.
- Breaking changes must be clearly marked with a `BREAKING` section.
- Release entry must include:
  - version,
  - date,
  - compatibility notes (Node/OS matrix changes if any),
  - migration note when behavior changes.

## Hotfix releases

For `patch` hotfixes:

- include only targeted fixes;
- avoid refactors without user impact;
- keep changelog concise and explicit about regression scope.
