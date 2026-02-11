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
      'output_dir = "fdocs/node"',
      "prune = false",
      'docs_source = "github"',
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

  mkdirSync(join(root, "fdocs/node"), { recursive: true });
  return root;
}

function createMultiFixtureRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "aifd-sync-partial-"));
  writeFileSync(
    join(root, "ai-fdocs.toml"),
    [
      "[settings]",
      'output_dir = "fdocs/node"',
      "prune = false",
      'docs_source = "github"',
      "",
      "[packages.lodash]",
      'repo = "lodash/lodash"',
      "",
      "[packages.axios]",
      'repo = "axios/axios"',
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
        "node_modules/axios": { version: "1.7.0" },
      },
    }),
    "utf-8"
  );

  mkdirSync(join(root, "fdocs/node"), { recursive: true });
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
      join(root, "fdocs/node/lodash@4.17.21/.aifd-meta.toml"),
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

  it("falls back to npm tarball when GitHub file fetch throws", async () => {
    const root = createFixtureRoot();
    const logs: string[] = [];

    vi.spyOn(console, "log").mockImplementation((msg?: unknown) => logs.push(String(msg ?? "")));
    vi.spyOn(GitHubClient.prototype, "resolveRef").mockResolvedValue({ gitRef: "v4.17.21", isFallback: false });
    vi.spyOn(GitHubClient.prototype, "fetchDefaultFiles").mockRejectedValue(new AiDocsError("rate limited", "GITHUB_RATE_LIMIT"));
    vi.spyOn(NpmRegistryClient.prototype, "getTarballUrl").mockResolvedValue("https://registry.example/lodash.tgz");
    vi.spyOn(fetcher, "fetchDocsFromNpmTarball").mockResolvedValue([{ path: "README.md", content: "# docs" }]);

    await cmdSync(root, false, "json");

    const report = JSON.parse(logs.at(-1) ?? "{}");
    expect(report.sourceStats.npm_tarball.synced).toBe(1);
    expect(report.totals.errors).toBe(0);
  });

  it("returns error when both GitHub fetch and npm fallback fail", async () => {
    const root = createFixtureRoot();
    const logs: string[] = [];

    vi.spyOn(console, "log").mockImplementation((msg?: unknown) => logs.push(String(msg ?? "")));
    vi.spyOn(GitHubClient.prototype, "resolveRef").mockResolvedValue({ gitRef: "v4.17.21", isFallback: false });
    vi.spyOn(GitHubClient.prototype, "fetchDefaultFiles").mockRejectedValue(new AiDocsError("rate limited", "GITHUB_RATE_LIMIT"));
    vi.spyOn(NpmRegistryClient.prototype, "getTarballUrl").mockResolvedValue(null);

    await cmdSync(root, false, "json");

    const report = JSON.parse(logs.at(-1) ?? "{}");
    expect(report.totals.errors).toBe(1);
    expect(report.errorCodes).toEqual({ GITHUB_RATE_LIMIT: 1 });
    expect(report.issues[0]).toContain("GitHub fetch failed");
    expect(report.issues[0]).toContain("npm fallback failed");
  });


  it("reports fallback failure details when GitHub and empty-result fallback both fail", async () => {
    const root = createFixtureRoot();
    const logs: string[] = [];

    vi.spyOn(console, "log").mockImplementation((msg?: unknown) => logs.push(String(msg ?? "")));
    vi.spyOn(GitHubClient.prototype, "resolveRef").mockResolvedValue({ gitRef: "v4.17.21", isFallback: false });
    vi.spyOn(GitHubClient.prototype, "fetchDefaultFiles").mockResolvedValue([]);
    vi.spyOn(NpmRegistryClient.prototype, "getTarballUrl").mockResolvedValue(null);

    await cmdSync(root, false, "json");

    const report = JSON.parse(logs.at(-1) ?? "{}");
    expect(report.totals.skipped).toBe(1);
    expect(report.issues[0]).toContain("no files found");
    expect(report.issues[0]).toContain("npm fallback failed (no npm tarball URL)");
  });


  it("keeps best-effort behavior on partial failures", async () => {
    const root = createMultiFixtureRoot();
    const logs: string[] = [];

    vi.spyOn(console, "log").mockImplementation((msg?: unknown) => logs.push(String(msg ?? "")));
    vi.spyOn(GitHubClient.prototype, "resolveRef").mockImplementation(async (repo: string) => {
      if (repo === "lodash/lodash") return { gitRef: "v4.17.21", isFallback: false };
      return { gitRef: "v1.7.0", isFallback: false };
    });
    vi.spyOn(GitHubClient.prototype, "fetchDefaultFiles").mockImplementation(async (repo: string) => {
      if (repo === "lodash/lodash") {
        throw new AiDocsError("rate limited", "GITHUB_RATE_LIMIT");
      }
      return [{ path: "README.md", content: "# axios docs" }];
    });
    vi.spyOn(NpmRegistryClient.prototype, "getTarballUrl").mockResolvedValue(null);

    await cmdSync(root, false, "json");

    const report = JSON.parse(logs.at(-1) ?? "{}");
    expect(report.totals.synced).toBe(1);
    expect(report.totals.errors).toBe(1);
    expect(report.errorCodes).toEqual({ GITHUB_RATE_LIMIT: 1 });

    const index = readFileSync(join(root, "fdocs/node/_INDEX.md"), "utf-8");
    expect(index).toContain("[axios@1.7.0](axios@1.7.0/_SUMMARY.md)");
    expect(index).not.toContain("lodash@4.17.21");
  });


  it("does not re-run npm fallback after it already returned empty files", async () => {
    const root = createFixtureRoot();
    const logs: string[] = [];

    vi.spyOn(console, "log").mockImplementation((msg?: unknown) => logs.push(String(msg ?? "")));
    vi.spyOn(GitHubClient.prototype, "resolveRef").mockRejectedValue(new AiDocsError("No suitable git ref found", "NO_REF"));
    const tarballUrlSpy = vi.spyOn(NpmRegistryClient.prototype, "getTarballUrl").mockResolvedValue("https://registry.example/lodash.tgz");
    vi.spyOn(fetcher, "fetchDocsFromNpmTarball").mockResolvedValue([]);

    await cmdSync(root, false, "json");

    const report = JSON.parse(logs.at(-1) ?? "{}");
    expect(report.totals.skipped).toBe(1);
    expect(tarballUrlSpy).toHaveBeenCalledTimes(1);
  });

});
