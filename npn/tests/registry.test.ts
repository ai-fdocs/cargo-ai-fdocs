import { describe, it, expect } from "vitest";
import { extractGithubRepo } from "../src/registry.js";

describe("extractGithubRepo", () => {
  it("parses simple https URL", () => {
    const result = extractGithubRepo("https://github.com/expressjs/express");
    expect(result?.repo).toBe("expressjs/express");
    expect(result?.subpath).toBeUndefined();
  });

  it("parses URL with .git suffix", () => {
    const result = extractGithubRepo("https://github.com/lodash/lodash.git");
    expect(result?.repo).toBe("lodash/lodash");
  });

  it("parses git+ prefix", () => {
    const result = extractGithubRepo("git+https://github.com/colinhacks/zod.git");
    expect(result?.repo).toBe("colinhacks/zod");
  });

  it("parses git:// protocol", () => {
    const result = extractGithubRepo("git://github.com/joyent/node.git");
    expect(result?.repo).toBe("joyent/node");
  });

  it("parses github: shorthand", () => {
    const result = extractGithubRepo("github:expressjs/express");
    expect(result?.repo).toBe("expressjs/express");
  });

  it("parses owner/repo shorthand", () => {
    const result = extractGithubRepo("expressjs/express");
    expect(result?.repo).toBe("expressjs/express");
  });

  it("parses monorepo subpath", () => {
    const result = extractGithubRepo("https://github.com/trpc/trpc/tree/main/packages/server");
    expect(result?.repo).toBe("trpc/trpc");
    expect(result?.subpath).toBe("packages/server");
  });

  it("returns null for non-GitHub URL", () => {
    expect(extractGithubRepo("https://gitlab.com/foo/bar")).toBeNull();
  });

  it("handles ssh format", () => {
    const result = extractGithubRepo("git@github.com:user/repo.git");
    expect(result?.repo).toBe("user/repo");
  });

  it("returns null for invalid input", () => {
    expect(extractGithubRepo("not a url at all")).toBeNull();
  });
});
