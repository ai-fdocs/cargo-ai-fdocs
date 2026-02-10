import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import YAML from "yaml";
import { AiDocsError } from "./error.js";

export function resolveVersions(projectRoot: string): Map<string, string> {
  const packageLock = join(projectRoot, "package-lock.json");
  const pnpmLock = join(projectRoot, "pnpm-lock.yaml");
  const yarnLock = join(projectRoot, "yarn.lock");

  if (existsSync(packageLock)) {
    return fromPackageLock(readFileSync(packageLock, "utf-8"));
  }
  if (existsSync(pnpmLock)) {
    return fromPnpmLock(readFileSync(pnpmLock, "utf-8"));
  }
  if (existsSync(yarnLock)) {
    return fromYarnLock(readFileSync(yarnLock, "utf-8"));
  }

  throw new AiDocsError(
    "No supported lockfile found (package-lock.json, pnpm-lock.yaml, yarn.lock)",
    "LOCKFILE_NOT_FOUND"
  );
}

function fromPackageLock(raw: string): Map<string, string> {
  const out = new Map<string, string>();
  const data = JSON.parse(raw) as {
    packages?: Record<string, { version?: string }>;
    dependencies?: Record<string, { version?: string }>;
  };

  if (data.packages) {
    for (const [path, info] of Object.entries(data.packages)) {
      if (!path.startsWith("node_modules/") || !info.version) continue;
      const name = path.replace(/^node_modules\//, "");
      out.set(name, info.version);
    }
  }

  if (data.dependencies) {
    for (const [name, info] of Object.entries(data.dependencies)) {
      if (info.version && !out.has(name)) out.set(name, info.version);
    }
  }

  return out;
}

function fromPnpmLock(raw: string): Map<string, string> {
  const out = new Map<string, string>();
  const data = YAML.parse(raw) as {
    packages?: Record<string, unknown>;
  };

  for (const key of Object.keys(data.packages ?? {})) {
    const cleaned = key.replace(/^\//, "");
    const at = cleaned.lastIndexOf("@");
    if (at <= 0) continue;
    const name = cleaned.slice(0, at);
    const version = cleaned.slice(at + 1).split("(")[0];
    if (name && version && !out.has(name)) out.set(name, version);
  }

  return out;
}

function fromYarnLock(raw: string): Map<string, string> {
  const out = new Map<string, string>();
  const blocks = raw.split(/\n{2,}/);
  for (const block of blocks) {
    const versionMatch = block.match(/\n\s*version\s+"([^"]+)"/);
    if (!versionMatch) continue;

    const header = block.split("\n", 1)[0]?.trim();
    if (!header) continue;
    const selector = header.split(",")[0]?.trim();
    if (!selector) continue;

    const pkg = selector.replace(/^"|"$/g, "");
    const at = pkg.lastIndexOf("@");
    if (at <= 0) continue;
    const name = pkg.slice(0, at);
    const version = versionMatch[1];
    if (!out.has(name)) out.set(name, version);
  }

  return out;
}
