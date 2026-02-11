import { describe, it, expect } from "vitest";
import { computeConfigHash } from "../src/config-hash.js";

describe("computeConfigHash", () => {
  it("is stable regardless of files order", () => {
    const a = computeConfigHash({
      repo: "foo/bar",
      subpath: "docs",
      files: ["README.md", "guides/setup.md"],
    });

    const b = computeConfigHash({
      repo: "foo/bar",
      subpath: "docs",
      files: ["guides/setup.md", "README.md"],
    });

    expect(a).toBe(b);
  });

  it("normalizes subpath separators and surrounding slashes", () => {
    const a = computeConfigHash({
      repo: "foo/bar",
      subpath: "docs/api",
    });

    const b = computeConfigHash({
      repo: "foo/bar",
      subpath: "/docs\\api/",
    });

    expect(a).toBe(b);
  });
});
