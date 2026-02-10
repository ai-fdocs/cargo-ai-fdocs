import { createHash } from "node:crypto";
import type { PackageConfig } from "./config.js";

export function computeConfigHash(pkgConfig: PackageConfig): string {
  const hash = createHash("sha256");
  hash.update(pkgConfig.repo);
  hash.update(pkgConfig.subpath ?? "");
  hash.update(JSON.stringify(pkgConfig.files ?? []));
  return hash.digest("hex").slice(0, 16);
}
