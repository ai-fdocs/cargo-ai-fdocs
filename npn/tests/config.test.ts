import { describe, expect, it } from "vitest";
import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { loadConfig } from "../src/config.js";

describe("loadConfig docs_source", () => {
  it("uses npm_tarball by default when source settings are omitted", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const cfg = loadConfig(root);
    expect(cfg.settings.docs_source).toBe("npm_tarball");
  });

  it("keeps backward compatibility with legacy experimental_npm_tarball=false", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'experimental_npm_tarball = false', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const cfg = loadConfig(root);
    expect(cfg.settings.docs_source).toBe("github");
  });
});
