import { afterEach, describe, expect, it, vi } from "vitest";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import * as fetcher from "../src/fetcher.js";
import { NpmRegistryClient } from "../src/registry.js";
import { GitHubClient } from "../src/fetcher.js";
import { cmdSync } from "../src/commands/sync.js";
import { AiDocsError } from "../src/error.js";

function createFixtureRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "aifd-sync-fallback-"));
  writeFileSync(
    join(root, "ai-fdocs.toml"),
    [
      "[settings]",
      'output_dir = "docs/ai/vendor-docs/node"',
      "prune = false",
      "",
      "[packages.lodash]",
      'repo = "lodash/lodash"',
    ].join("\n"),
    "utf-8"
  );

  writeFileSync(
    join(root, "package-lock.json"),
    JSON.stringify({
      name: "fixture",
      lockfileVersion: 3,
      packages: {
        "": { name: "fixture", version: "1.0.0" },
        "node_modules/lodash": { version: "4.17.21" },
      },
    }),
    "utf-8"
  );

  mkdirSync(join(root, "docs/ai/vendor-docs/node"), { recursive: true });
  return root;
}

describe("cmdSync github fallback", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("falls back to npm tarball when GitHub ref resolution fails", async () => {
    const root = createFixtureRoot();
    const logs: string[] = [];

    vi.spyOn(console, "log").mockImplementation((msg?: unknown) => logs.push(String(msg ?? "")));
    vi.spyOn(GitHubClient.prototype, "resolveRef").mockRejectedValue(new AiDocsError("No suitable git ref found", "NO_REF"));
    vi.spyOn(NpmRegistryClient.prototype, "getTarballUrl").mockResolvedValue("https://registry.example/lodash.tgz");
    vi.spyOn(fetcher, "fetchDocsFromNpmTarball").mockResolvedValue([{ path: "README.md", content: "# docs" }]);

    await cmdSync(root, false, "json");

    const report = JSON.parse(logs.at(-1) ?? "{}");
    expect(report.source).toBe("github");
    expect(report.sourceStats.npm_tarball.synced).toBe(1);
    expect(report.sourceStats.github.synced).toBe(0);

    const meta = readFileSync(
      join(root, "docs/ai/vendor-docs/node/lodash@4.17.21/.aifd-meta.toml"),
      "utf-8"
    );
    expect(meta).toContain('git_ref = "npm-tarball"');
  });

  it("falls back to npm tarball when GitHub files list is empty", async () => {
    const root = createFixtureRoot();
    const logs: string[] = [];

    vi.spyOn(console, "log").mockImplementation((msg?: unknown) => logs.push(String(msg ?? "")));
    vi.spyOn(GitHubClient.prototype, "resolveRef").mockResolvedValue({ gitRef: "v4.17.21", isFallback: false });
    vi.spyOn(GitHubClient.prototype, "fetchDefaultFiles").mockResolvedValue([]);
    vi.spyOn(NpmRegistryClient.prototype, "getTarballUrl").mockResolvedValue("https://registry.example/lodash.tgz");
    vi.spyOn(fetcher, "fetchDocsFromNpmTarball").mockResolvedValue([{ path: "README.md", content: "# docs" }]);

    await cmdSync(root, false, "json");

    const report = JSON.parse(logs.at(-1) ?? "{}");
    expect(report.sourceStats.npm_tarball.synced).toBe(1);
    expect(report.totals.synced).toBe(1);
  });
});
