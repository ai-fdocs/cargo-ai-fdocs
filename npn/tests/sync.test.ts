import { describe, expect, it } from "vitest";
import { summarizeSourceStats } from "../src/commands/sync.js";

describe("summarizeSourceStats", () => {
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
});
