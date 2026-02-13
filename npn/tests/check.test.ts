import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, it } from "vitest";
import { buildCheckReport, renderJsonReport } from "../src/commands/check.js";

describe("check report", () => {
  it("returns machine-readable report with missing docs issue", async () => {
    const root = mkdtempSync(join(tmpdir(), "aifd-check-"));

    writeFileSync(
      join(root, "ai-fdocs.toml"),
      [
        "[settings]",
        'output_dir = "fdocs/node"',
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

    const report = await buildCheckReport(root);

    expect(report.ok).toBe(false);
    expect(report.issues).toEqual([{ name: "lodash", kind: "missing" }]);

    const parsed = JSON.parse(renderJsonReport(report));
    expect(parsed).toEqual({ ok: false, issues: [{ name: "lodash", kind: "missing" }] });
  });
});
