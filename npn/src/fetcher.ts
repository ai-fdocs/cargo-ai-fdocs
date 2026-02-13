import { AiDocsError } from "./error.js";
import { classifyHttpError, fetchWithRetry } from "./net.js";
import { createGunzip } from "node:zlib";
import { Readable } from "node:stream";
import tar, { type Headers } from "tar-stream";

export interface ResolvedRef {
  gitRef: string;
  isFallback: boolean;
}

export interface FetchedFile {
  path: string;
  content: string;
}

interface GitTreeResponse {
  tree?: Array<{ path: string; type: string }>;
}

export class GitHubClient {
  private token = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN;

  async resolveRef(repo: string, _name: string, version: string): Promise<ResolvedRef> {
    const candidates = [`v${version}`, version];

    for (const ref of candidates) {
      if (await this.refExists(repo, ref)) {
        return { gitRef: ref.replace("refs/tags/", ""), isFallback: false };
      }
    }

    if (await this.refExists(repo, "main")) {
      return { gitRef: "main", isFallback: true };
    }
    if (await this.refExists(repo, "master")) {
      return { gitRef: "master", isFallback: true };
    }

    throw new AiDocsError(`No suitable git ref found for ${repo}`, "NO_REF");
  }

  async fetchExplicitFiles(repo: string, ref: string, files: string[]): Promise<FetchedFile[]> {
    const out: FetchedFile[] = [];
    for (const file of files) {
      const content = await this.fetchRaw(repo, ref, file);
      if (content !== null) out.push({ path: file, content });
    }
    return out;
  }

  async fetchDefaultFiles(repo: string, ref: string, subpath?: string): Promise<FetchedFile[]> {
    const tree = await this.fetchTree(repo, ref);
    const base = subpath ? `${subpath.replace(/\/$/, "")}/` : "";
    const preferred = new Set([
      "readme.md",
      "changelog.md",
      "changes.md",
      "history.md",
      "license",
      "license.md",
      "index.html",
      "docs/readme.md",
    ]);

    const picked = tree
      .filter((f) => f.type === "blob")
      .filter((f) => !base || f.path.startsWith(base))
      .map((f) => ({ ...f, rel: base ? f.path.slice(base.length) : f.path }))
      .filter((f) => {
        const lower = f.rel.toLowerCase();
        if (preferred.has(lower)) return true;
        return lower.startsWith("docs/") && lower.endsWith(".md");
      })
      .slice(0, 40);

    const out: FetchedFile[] = [];
    for (const file of picked) {
      const content = await this.fetchRaw(repo, ref, file.path);
      if (content !== null) out.push({ path: file.rel, content });
    }
    return out;
  }

  private async refExists(repo: string, ref: string): Promise<boolean> {
    const normalized = ref.replace(/^refs\//, "");
    const targets = normalized.includes("/") ? [normalized] : [`heads/${normalized}`, `tags/${normalized}`];

    for (const target of targets) {
      const url = `https://api.github.com/repos/${repo}/git/ref/${target}`;
      const resp = await fetchWithRetry(url, { headers: this.headers() });
      if (resp.ok) return true;
    }

    return false;
  }

  private async fetchTree(repo: string, ref: string): Promise<Array<{ path: string; type: string }>> {
    const url = `https://api.github.com/repos/${repo}/git/trees/${encodeURIComponent(ref)}?recursive=1`;
    const resp = await fetchWithRetry(url, { headers: this.headers() });
    if (!resp.ok) {
      const kind = classifyHttpError(resp.status);
      throw new AiDocsError(`Failed to fetch file tree for ${repo}@${ref}: ${resp.status}`, `GITHUB_${kind.toUpperCase()}`);
    }
    const data = (await resp.json()) as GitTreeResponse;
    return data.tree ?? [];
  }

  private async fetchRaw(repo: string, ref: string, filePath: string): Promise<string | null> {
    const url = `https://raw.githubusercontent.com/${repo}/${encodeURIComponent(ref)}/${filePath}`;
    const resp = await fetchWithRetry(url, { headers: this.headers() });
    if (resp.status === 404) return null;
    if (!resp.ok) {
      const kind = classifyHttpError(resp.status);
      throw new AiDocsError(`Failed to fetch file ${filePath} from ${repo}@${ref}: ${resp.status}`, `GITHUB_${kind.toUpperCase()}`);
    }
    return resp.text();
  }

  private headers(): Record<string, string> {
    return {
      Accept: "application/vnd.github+json",
      ...(this.token ? { Authorization: `Bearer ${this.token}` } : {}),
      "User-Agent": "ai-fdocs/0.2.0",
    };
  }
}

export async function fetchDocsFromNpmTarball(
  tarballUrl: string,
  subpath?: string,
  explicitFiles?: string[]
): Promise<FetchedFile[]> {
  const resp = await fetchWithRetry(tarballUrl);
  if (!resp.ok) {
    const kind = classifyHttpError(resp.status);
    throw new AiDocsError(`Failed to download npm tarball: ${resp.status}`, `NPM_TARBALL_${kind.toUpperCase()}`);
  }

  const raw = Buffer.from(await resp.arrayBuffer());
  const extract = tar.extract();
  const out: FetchedFile[] = [];

  const normalizedSubpath = subpath ? `${subpath.replace(/^\/+|\/+$/g, "")}/` : "";
  const explicitSet = explicitFiles ? new Set(explicitFiles) : null;
  const preferred = new Set([
    "readme.md",
    "changelog.md",
    "changes.md",
    "history.md",
    "license",
    "license.md",
    "index.html",
    "docs/readme.md",
  ]);

  await new Promise<void>((resolve, reject) => {
    extract.on("entry", (header: Headers, stream: NodeJS.ReadableStream, next: () => void) => {
      if (header.type !== "file") {
        stream.resume();
        stream.on("end", next);
        return;
      }

      const fullPath = header.name.replace(/^package\//, "");
      if (!fullPath || fullPath.startsWith("/") || fullPath.includes("..")) {
        stream.resume();
        stream.on("end", next);
        return;
      }
      const relPath = normalizedSubpath && fullPath.startsWith(normalizedSubpath)
        ? fullPath.slice(normalizedSubpath.length)
        : fullPath;

      const shouldInclude = (() => {
        if (explicitSet) return explicitSet.has(relPath);
        if (normalizedSubpath && !fullPath.startsWith(normalizedSubpath)) return false;
        const lower = relPath.toLowerCase();
        if (preferred.has(lower)) return true;
        return lower.startsWith("docs/") && lower.endsWith(".md");
      })();

      if (!shouldInclude) {
        stream.resume();
        stream.on("end", next);
        return;
      }

      const chunks: Buffer[] = [];
      stream.on("data", (chunk: Buffer) => chunks.push(chunk));
      stream.on("error", reject);
      stream.on("end", () => {
        out.push({ path: relPath, content: Buffer.concat(chunks).toString("utf-8") });
        next();
      });
    });

    extract.on("finish", resolve);
    extract.on("error", reject);

    Readable.from(raw)
      .pipe(createGunzip())
      .on("error", reject)
      .pipe(extract)
      .on("error", reject);
  });

  return out.slice(0, 40);
}
