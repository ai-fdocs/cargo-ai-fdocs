import { describe, expect, it } from "vitest";
import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { loadConfig } from "../src/config.js";
import { AiDocsError } from "../src/error.js";

describe("loadConfig settings validation", () => {
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

  it("uses explicit docs_source when provided", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'docs_source = "github"', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const cfg = loadConfig(root);
    expect(cfg.settings.docs_source).toBe("github");
  });

  it("fails fast on invalid docs_source", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'docs_source = "gitlab"', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const load = () => loadConfig(root);
    expect(load).toThrowError(AiDocsError);
    expect(load).toThrowError(/settings\.docs_source must be "github" or "npm_tarball"/);
  });

  it("uses default max_file_size_kb=512", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(join(root, "ai-fdocs.toml"), ['[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"), "utf-8");

    const cfg = loadConfig(root);
    expect(cfg.settings.max_file_size_kb).toBe(512);
  });

  it("fails fast on invalid max_file_size_kb", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'max_file_size_kb = 0', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const load = () => loadConfig(root);
    expect(load).toThrowError(AiDocsError);
    expect(load).toThrowError(/settings\.max_file_size_kb must be a positive integer/);
  });

  it("fails fast on non-integer max_file_size_kb", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'max_file_size_kb = 1.5', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const load = () => loadConfig(root);
    expect(load).toThrowError(AiDocsError);
    expect(load).toThrowError(/settings\.max_file_size_kb must be a positive integer/);
  });

  it("fails fast on non-numeric max_file_size_kb", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'max_file_size_kb = true', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const load = () => loadConfig(root);
    expect(load).toThrowError(AiDocsError);
    expect(load).toThrowError(/settings\.max_file_size_kb must be a positive integer/);
  });

  it("uses default sync_concurrency=8", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(join(root, "ai-fdocs.toml"), ['[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"), "utf-8");

    const cfg = loadConfig(root);
    expect(cfg.settings.sync_concurrency).toBe(8);
  });

  it("fails fast on invalid sync_concurrency", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'sync_concurrency = 0', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const load = () => loadConfig(root);
    expect(load).toThrowError(AiDocsError);
    expect(load).toThrowError(/settings\.sync_concurrency must be a positive integer/);
  });

  it("fails fast on non-integer sync_concurrency", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'sync_concurrency = 1.5', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const load = () => loadConfig(root);
    expect(load).toThrowError(AiDocsError);
    expect(load).toThrowError(/settings\.sync_concurrency must be a positive integer/);
  });

  it("fails fast on non-numeric sync_concurrency", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'sync_concurrency = true', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const load = () => loadConfig(root);
    expect(load).toThrowError(AiDocsError);
    expect(load).toThrowError(/settings\.sync_concurrency must be a positive integer/);
  });

  it("fails fast on string sync_concurrency (no implicit coercion)", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'sync_concurrency = "8"', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const load = () => loadConfig(root);
    expect(load).toThrowError(AiDocsError);
    expect(load).toThrowError(/settings\.sync_concurrency must be a positive integer/);
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

  it("keeps backward compatibility with legacy experimental_npm_tarball=true", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      ['[settings]', 'experimental_npm_tarball = true', '', '[packages.lodash]', 'repo = "lodash/lodash"'].join("\n"),
      "utf-8"
    );

    const cfg = loadConfig(root);
    expect(cfg.settings.docs_source).toBe("npm_tarball");
  });

  it("prefers explicit docs_source over legacy experimental_npm_tarball", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-config-"));
    writeFileSync(
      join(root, "ai-fdocs.toml"),
      [
        '[settings]',
        'docs_source = "github"',
        'experimental_npm_tarball = true',
        '',
        '[packages.lodash]',
        'repo = "lodash/lodash"',
      ].join("\n"),
      "utf-8"
    );

    const cfg = loadConfig(root);
    expect(cfg.settings.docs_source).toBe("github");
  });

});
