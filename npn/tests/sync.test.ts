import { describe, expect, it } from "vitest";
import { buildSyncReport, summarizeErrorCodes, summarizeSourceStats } from "../src/commands/sync.js";

describe("sync summaries", () => {
  it("aggregates statuses by source", () => {
    const stats = summarizeSourceStats([
      { saved: null, status: "cached", source: "github" },
      { saved: null, status: "error", source: "github", message: "boom" },
      { saved: null, status: "skipped", source: "github", message: "skip" },
      {
        saved: {
          name: "lodash",
          version: "4.17.21",
          gitRef: "v4.17.21",
          isFallback: false,
          files: ["README.md"],
          aiNotes: "",
        },
        status: "synced",
        source: "npm_tarball",
      },
    ]);

    expect(stats.github).toEqual({ synced: 0, cached: 1, skipped: 1, errors: 1 });
    expect(stats.npm_tarball).toEqual({ synced: 1, cached: 0, skipped: 0, errors: 0 });
  });

  it("aggregates error codes for short summary", () => {
    const counts = summarizeErrorCodes([
      { saved: null, status: "error", source: "github", message: "auth", errorCode: "GITHUB_AUTH" },
      { saved: null, status: "error", source: "github", message: "rate", errorCode: "GITHUB_RATE_LIMIT" },
      { saved: null, status: "error", source: "github", message: "auth2", errorCode: "GITHUB_AUTH" },
      { saved: null, status: "cached", source: "github" },
    ]);

    expect(counts).toEqual({ GITHUB_AUTH: 2, GITHUB_RATE_LIMIT: 1 });
  });

  it("builds machine-readable sync report", () => {
    const report = buildSyncReport(
      [
        { saved: null, status: "cached", source: "github" },
        { saved: null, status: "error", source: "github", errorCode: "GITHUB_AUTH" },
        { saved: null, status: "skipped", source: "github" },
      ],
      "github"
    );

    expect(report.source).toBe("github");
    expect(report.totals).toEqual({ synced: 0, cached: 1, skipped: 1, errors: 1 });
    expect(report.sourceStats.github.errors).toBe(1);
    expect(report.errorCodes).toEqual({ GITHUB_AUTH: 1 });
    expect(report.issues).toEqual([]);
  });
});
