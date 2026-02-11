import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, it, expect } from "vitest";
import { isCachedV2 } from "../src/storage.js";

describe("isCachedV2 metadata compatibility", () => {
  it("accepts legacy metadata without schema_version", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-legacy-"));
    const dir = join(root, "pkg@1.0.0");
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      join(dir, ".aifd-meta.toml"),
      'version = "1.0.0"\n' +
        'git_ref = "v1.0.0"\n' +
        'fetched_at = "2026-01-01"\n' +
        'is_fallback = false\n' +
        'config_hash = "abcd"\n',
      "utf-8"
    );

    expect(isCachedV2(root, "pkg", "1.0.0", "abcd")).toBe(true);
  });

  it("returns false when config hash mismatches", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-v2-"));
    const dir = join(root, "pkg@1.0.0");
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      join(dir, ".aifd-meta.toml"),
      'schema_version = 2\n' +
        'version = "1.0.0"\n' +
        'git_ref = "v1.0.0"\n' +
        'fetched_at = "2026-01-01"\n' +
        'is_fallback = false\n' +
        'config_hash = "abcd"\n',
      "utf-8"
    );

    expect(isCachedV2(root, "pkg", "1.0.0", "efgh")).toBe(false);
  });
});
