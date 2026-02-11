import { mkdtempSync, mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, it } from "vitest";
import { generateSummary } from "../src/summary.js";
import { generateIndex } from "../src/index.js";

describe("summary/index rendering", () => {
  it("renders fallback summary with AI notes and file table", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-summary-"));
    const pkgDir = join(root, "lodash@4.17.21");
    mkdirSync(pkgDir, { recursive: true });

    generateSummary(pkgDir, {
      packageName: "lodash",
      version: "4.17.21",
      repo: "lodash/lodash",
      gitRef: "main",
      isFallback: true,
      aiNotes: "Use docs carefully.",
      files: [
        { flatName: "README.md", originalPath: "README.md", sizeBytes: 999 },
        { flatName: "docs_api.md", originalPath: "docs/api.md", sizeBytes: 2300 },
      ],
    });

    const summary = readFileSync(join(pkgDir, "_SUMMARY.md"), "utf-8");

    expect(summary).toContain("fallback — no tag found for this version");
    expect(summary).toContain("## AI Notes");
    expect(summary).toContain("| [README.md](README.md) | `README.md` | 999B |");
    expect(summary).toContain("| [docs_api.md](docs_api.md) | `docs/api.md` | 2.2KB |");
  });

  it("renders index with fallback marker", () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-index-"));

    generateIndex(root, [
      {
        name: "axios",
        version: "1.7.0",
        gitRef: "v1.7.0",
        isFallback: false,
        files: ["README.md"],
        aiNotes: "",
      },
      {
        name: "left-pad",
        version: "1.3.0",
        gitRef: "main",
        isFallback: true,
        files: ["README.md"],
        aiNotes: "",
      },
    ]);

    const index = readFileSync(join(root, "_INDEX.md"), "utf-8");

    expect(index).toContain("[axios@1.7.0](axios@1.7.0/_SUMMARY.md)");
    expect(index).toContain("[left-pad@1.3.0](left-pad@1.3.0/_SUMMARY.md) ⚠️ fallback");
  });
});
