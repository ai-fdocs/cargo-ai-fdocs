import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import type { SavedPackage } from "./storage.js";

export function generateIndex(outputDir: string, packages: SavedPackage[]): void {
  mkdirSync(outputDir, { recursive: true });

  let content = "# AI Vendor Docs Index\n\n";
  if (packages.length === 0) {
    content += "No packages were synced.\n";
  } else {
    for (const pkg of packages) {
      const dir = `${pkg.name}@${pkg.version}`;
      const suffix = pkg.isFallback ? " ⚠️ fallback" : "";
      content += `- [${pkg.name}@${pkg.version}](${dir}/_SUMMARY.md)${suffix}\n`;
    }
  }

  writeFileSync(join(outputDir, "_INDEX.md"), content, "utf-8");
}
