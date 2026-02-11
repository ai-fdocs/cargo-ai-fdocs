import { afterEach, describe, expect, it, vi } from "vitest";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { cmdInit } from "../src/commands/init.js";
import { NpmRegistryClient } from "../src/registry.js";

function createRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "aifd-init-"));
  writeFileSync(
    join(root, "package.json"),
    JSON.stringify({
      name: "fixture",
      version: "1.0.0",
      dependencies: {
        lodash: "^4.17.21",
      },
    }),
    "utf-8"
  );

  return root;
}

describe("cmdInit", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("generates settings with sync_concurrency and docs_source defaults", async () => {
    const root = createRoot();

    vi.spyOn(console, "log").mockImplementation(() => undefined);
    vi.spyOn(NpmRegistryClient.prototype, "getPackageInfo").mockResolvedValue({
      repository: "https://github.com/lodash/lodash",
      description: "lodash docs",
    });

    await cmdInit(root, false);

    const config = readFileSync(join(root, "ai-fdocs.toml"), "utf-8");
    expect(config).toContain("sync_concurrency = 8");
    expect(config).toContain('docs_source = "npm_tarball"');
    expect(config).toContain('[packages.lodash]');
  });
});
