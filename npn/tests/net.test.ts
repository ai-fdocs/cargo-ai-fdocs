import { afterEach, describe, expect, it, vi } from "vitest";
import { classifyHttpError, fetchWithRetry } from "../src/net.js";

describe("net helpers", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("classifies known HTTP statuses", () => {
    expect(classifyHttpError(401)).toBe("auth");
    expect(classifyHttpError(403)).toBe("auth");
    expect(classifyHttpError(404)).toBe("not_found");
    expect(classifyHttpError(429)).toBe("rate_limit");
    expect(classifyHttpError(503)).toBe("server");
    expect(classifyHttpError(418)).toBe("unknown");
  });

  it("retries transient status codes", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response("busy", { status: 429 }))
      .mockResolvedValueOnce(new Response("ok", { status: 200 }));

    vi.stubGlobal("fetch", fetchMock);

    const resp = await fetchWithRetry("https://example.com", undefined, { attempts: 2, baseDelayMs: 1 });
    expect(resp.status).toBe(200);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});
