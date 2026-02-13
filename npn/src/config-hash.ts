import { createHash } from "node:crypto";
import type { PackageConfig } from "./config.js";

function normalizeSubpath(subpath?: string): string {
  if (!subpath) return "";
  return subpath
    .replace(/\\/g, "/")
    .split("/")
    .filter(Boolean)
    .join("/");
}

function normalizeFiles(files?: string[]): string[] {
  if (!files) return [];
  return [...files].sort((a, b) => a.localeCompare(b));
}

export function computeConfigHash(pkgConfig: PackageConfig): string {
  const hash = createHash("sha256");
  hash.update(pkgConfig.repo || "");
  hash.update(normalizeSubpath(pkgConfig.subpath));
  hash.update(JSON.stringify(normalizeFiles(pkgConfig.files)));
  return hash.digest("hex").slice(0, 16);
}
