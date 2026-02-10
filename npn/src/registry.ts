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
  dist?: {
    tarball?: string;
  };
}

export class NpmRegistryClient {
  private baseUrl = "https://registry.npmjs.org";

  async getPackageInfo(name: string): Promise<PackageInfo | null> {
    const encodedName = name.replace("/", "%2F");
    const url = `${this.baseUrl}/${encodedName}`;

    try {
      const resp = await fetch(url, { headers: { Accept: "application/json" } });

      if (resp.status === 404) return null;

      if (resp.status === 429) {
        await sleep(2000);
        const retry = await fetch(url);
        if (!retry.ok) return null;
        return parsePackageInfo((await retry.json()) as NpmPackageResponse);
      }

      if (!resp.ok) return null;
      return parsePackageInfo((await resp.json()) as NpmPackageResponse);
    } catch {
      return null;
    }
  }

  async getTarballUrl(name: string, version: string): Promise<string | null> {
    const encodedName = name.replace("/", "%2F");
    const encodedVersion = encodeURIComponent(version);
    const url = `${this.baseUrl}/${encodedName}/${encodedVersion}`;

    try {
      const resp = await fetch(url, { headers: { Accept: "application/json" } });
      if (!resp.ok) return null;
      const data = (await resp.json()) as NpmVersionResponse;
      return data.dist?.tarball ?? null;
    } catch {
      return null;
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
    const parsed = new URL(normalized);
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

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
