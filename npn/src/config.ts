import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { parse } from "smol-toml";
import { AiDocsError } from "./error.js";

export interface PackageConfig {
  repo: string;
  subpath?: string;
  files?: string[];
  ai_notes?: string;
}

export interface Config {
  settings: {
    output_dir: string;
    prune: boolean;
    max_file_size_kb: number;
    experimental_npm_tarball: boolean;
  };
  packages: Record<string, PackageConfig>;
}

export function loadConfig(projectRoot: string): Config {
  const configPath = join(projectRoot, "ai-fdocs.toml");
  if (!existsSync(configPath)) {
    throw new AiDocsError("ai-fdocs.toml not found", "CONFIG_NOT_FOUND");
  }

  const raw = readFileSync(configPath, "utf-8");
  const data = parse(raw) as Record<string, unknown>;

  const settings = (data.settings as Record<string, unknown> | undefined) ?? {};
  const packages = (data.packages as Record<string, PackageConfig> | undefined) ?? {};

  return {
    settings: {
      output_dir: String(settings.output_dir ?? "docs/ai/vendor-docs/node"),
      prune: Boolean(settings.prune ?? true),
      max_file_size_kb: Number(settings.max_file_size_kb ?? 512),
      experimental_npm_tarball: Boolean(settings.experimental_npm_tarball ?? false),
    },
    packages,
  };
}
