import { AiDocsError } from "./error.js";

export type ErrorKind = "auth" | "rate_limit" | "not_found" | "network" | "parse" | "server" | "unknown";

interface RetryOptions {
  attempts?: number;
  baseDelayMs?: number;
  retryOnStatuses?: number[];
}

const DEFAULT_RETRY_STATUSES = [408, 425, 429, 500, 502, 503, 504];

export function classifyHttpError(status: number): ErrorKind {
  if (status === 401 || status === 403) return "auth";
  if (status === 404) return "not_found";
  if (status === 429) return "rate_limit";
  if (status >= 500) return "server";
  return "unknown";
}

function shouldRetryStatus(status: number, retryOnStatuses: number[]): boolean {
  return retryOnStatuses.includes(status);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function fetchWithRetry(
  url: string,
  init?: RequestInit,
  opts: RetryOptions = {}
): Promise<Response> {
  const attempts = opts.attempts ?? 3;
  const baseDelayMs = opts.baseDelayMs ?? 250;
  const retryOnStatuses = opts.retryOnStatuses ?? DEFAULT_RETRY_STATUSES;

  let lastErr: unknown;

  for (let i = 0; i < attempts; i++) {
    try {
      const resp = await fetch(url, init);
      if (!shouldRetryStatus(resp.status, retryOnStatuses) || i === attempts - 1) return resp;
    } catch (err) {
      lastErr = err;
      if (i === attempts - 1) {
        throw new AiDocsError(`Network request failed for ${url}`, "NETWORK");
      }
    }

    const delay = baseDelayMs * 2 ** i;
    await sleep(delay);
  }

  throw new AiDocsError(`Network request failed for ${url}: ${String(lastErr)}`, "NETWORK");
}

export async function fetchJsonWithRetry<T>(url: string, init?: RequestInit, opts?: RetryOptions): Promise<T> {
  const resp = await fetchWithRetry(url, init, opts);
  try {
    return (await resp.json()) as T;
  } catch {
    throw new AiDocsError(`Failed to parse JSON response from ${url}`, "PARSE");
  }
}
