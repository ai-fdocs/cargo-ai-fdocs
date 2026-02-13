import { AiDocsError } from "./error.js";
import { classifyHttpError, fetchWithRetry } from "./net.js";

export interface PackageInfo {
  name: string;
  repository: string | null;
  subpath?: string;
  description: string | null;
}

interface NpmPackageResponse {
  name: string;
  description?: string;
  repository?:
  | {
    type?: string;
    url?: string;
    directory?: string;
  }
  | string;
}

interface NpmVersionResponse {
  version: string;
  readme?: string;
  dist?: {
    tarball?: string;
  };
}

export class NpmRegistryClient {
  private baseUrl = "https://registry.npmjs.org";

  async getPackageInfo(name: string): Promise<PackageInfo | null> {
    const encodedName = name.replace("/", "%2F");
    const url = `${this.baseUrl}/${encodedName}`;

    const resp = await fetchWithRetry(url, { headers: { Accept: "application/json" } });
    if (resp.status === 404) return null;
    if (!resp.ok) {
      const kind = classifyHttpError(resp.status);
      throw new AiDocsError(`npm registry request failed for ${name}: ${resp.status}`, `NPM_${kind.toUpperCase()}`);
    }

    try {
      const data = (await resp.json()) as NpmPackageResponse;
      return parsePackageInfo(data);
    } catch {
      throw new AiDocsError(`Failed to parse npm registry response for ${name}`, "NPM_PARSE");
    }
  }

  async getTarballUrl(name: string, version: string): Promise<string | null> {
    const encodedName = name.replace("/", "%2F");
    const encodedVersion = encodeURIComponent(version);
    const url = `${this.baseUrl}/${encodedName}/${encodedVersion}`;

    const resp = await fetchWithRetry(url, { headers: { Accept: "application/json" } });
    if (resp.status === 404) return null;
    if (!resp.ok) {
      const kind = classifyHttpError(resp.status);
      throw new AiDocsError(`npm registry tarball request failed for ${name}@${version}: ${resp.status}`, `NPM_${kind.toUpperCase()}`);
    }

    try {
      const data = (await resp.json()) as NpmVersionResponse;
      return data.dist?.tarball ?? null;
    } catch {
      throw new AiDocsError(`Failed to parse npm registry response for ${name}@${version}`, "NPM_PARSE");
    }
  }

  async getReadme(name: string, version: string): Promise<string | null> {
    const encodedName = name.replace("/", "%2F");
    const encodedVersion = encodeURIComponent(version);
    const url = `${this.baseUrl}/${encodedName}/${encodedVersion}`;

    const resp = await fetchWithRetry(url, { headers: { Accept: "application/json" } });
    if (resp.status === 404) return null;
    if (!resp.ok) {
      const kind = classifyHttpError(resp.status);
      throw new AiDocsError(`npm registry version metadata request failed for ${name}@${version}: ${resp.status}`, `NPM_${kind.toUpperCase()}`);
    }

    try {
      const data = (await resp.json()) as NpmVersionResponse;
      return data.readme ?? null;
    } catch {
      throw new AiDocsError(`Failed to parse npm registry response for ${name}@${version}`, "NPM_PARSE");
    }
  }

  async getLatestVersion(name: string): Promise<string> {
    const encodedName = name.replace("/", "%2F");
    const url = `${this.baseUrl}/${encodedName}/latest`;

    const resp = await fetchWithRetry(url, { headers: { Accept: "application/json" } });
    if (resp.status === 404) {
      throw new AiDocsError(`Package not found in npm registry: ${name}`, "NPM_NOT_FOUND");
    }
    if (!resp.ok) {
      const kind = classifyHttpError(resp.status);
      throw new AiDocsError(`npm registry latest version request failed for ${name}: ${resp.status}`, `NPM_${kind.toUpperCase()}`);
    }

    try {
      const data = (await resp.json()) as { version: string };
      if (!data.version) {
        throw new AiDocsError(`npm registry response for ${name}@latest has no version`, "NPM_PARSE");
      }
      return data.version;
    } catch {
      throw new AiDocsError(`Failed to parse npm registry response for ${name}@latest`, "NPM_PARSE");
    }
  }
}

function parsePackageInfo(data: NpmPackageResponse): PackageInfo {
  let repoUrl: string | null = null;
  let subpath: string | undefined;

  if (data.repository) {
    if (typeof data.repository === "string") {
      repoUrl = data.repository;
    } else if (data.repository.url) {
      repoUrl = data.repository.url;
      subpath = extractSubpathFromRepo(data.repository);
    }
  }

  return {
    name: data.name,
    repository: repoUrl,
    subpath,
    description: data.description ?? null,
  };
}

export function extractGithubRepo(url: string): { repo: string; subpath?: string } | null {
  let cleaned = url.trim();

  if (cleaned.startsWith("github:")) {
    return { repo: cleaned.replace("github:", "").trim() };
  }

  cleaned = cleaned.replace(/^git\+/, "");
  cleaned = cleaned
    .replace(/^git:\/\//, "https://")
    .replace(/^ssh:\/\/git@github\.com\//, "https://github.com/")
    .replace(/^git@github\.com:/, "https://github.com/");

  // short format: owner/repo
  if (!cleaned.includes("://") && cleaned.split("/").length === 2 && !cleaned.includes("@")) {
    const [owner, repo] = cleaned.split("/");
    if (owner && repo) return { repo: `${owner}/${repo}` };
  }

  if (!cleaned.includes("github.com")) return null;

  cleaned = cleaned
    .replace("https://github.com/", "")
    .replace("http://github.com/", "")
    .replace("git://github.com/", "")
    .replace("ssh://git@github.com/", "")
    .replace("git@github.com:", "");

  cleaned = cleaned.replace(/\.git$/, "").replace(/\/$/, "");

  try {
    const normalized = cleaned.includes("://") ? cleaned : `https://github.com/${cleaned}`;
    const parsed = new globalThis.URL(normalized);
    const parts = parsed.pathname.split("/").filter(Boolean);
    if (parts.length < 2) return null;

    const repo = `${parts[0]}/${parts[1]}`;
    let subpath: string | undefined;
    if (parts.length > 4 && parts[2] === "tree") {
      subpath = parts.slice(4).join("/");
    }
    return { repo, subpath };
  } catch {
    return null;
  }
}

export function extractSubpathFromRepo(repoField: unknown): string | undefined {
  if (
    repoField &&
    typeof repoField === "object" &&
    "directory" in repoField &&
    typeof (repoField as { directory?: unknown }).directory === "string"
  ) {
    return (repoField as { directory: string }).directory;
  }
  return undefined;
}

