import { afterEach, describe, expect, it, vi } from "vitest";
import { GitHubClient } from "../src/fetcher.js";
import { AiDocsError } from "../src/error.js";

describe("GitHubClient.resolveRef", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("uses matching tag when version tag exists", async () => {
    const fetchMock = vi.fn(async (url: string) => {
      if (url.includes("/git/ref/tags/v1.2.3")) return new Response("ok", { status: 200 });
      return new Response("missing", { status: 404 });
    });

    vi.stubGlobal("fetch", fetchMock);

    const client = new GitHubClient();
    const resolved = await client.resolveRef("owner/repo", "pkg", "1.2.3");

    expect(resolved).toEqual({ gitRef: "v1.2.3", isFallback: false });
  });

  it("falls back to main when no matching tag exists", async () => {
    const fetchMock = vi.fn(async (url: string) => {
      if (url.includes("/git/ref/heads/main")) return new Response("ok", { status: 200 });
      return new Response("missing", { status: 404 });
    });

    vi.stubGlobal("fetch", fetchMock);

    const client = new GitHubClient();
    const resolved = await client.resolveRef("owner/repo", "pkg", "1.2.3");

    expect(resolved).toEqual({ gitRef: "main", isFallback: true });
  });

  it("throws NO_REF when no tag/branch candidates exist", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("missing", { status: 404 })));

    const client = new GitHubClient();

    await expect(client.resolveRef("owner/repo", "pkg", "1.2.3")).rejects.toMatchObject<Partial<AiDocsError>>({
      code: "NO_REF",
    });
  });
});
