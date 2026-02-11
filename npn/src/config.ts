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

export type DocsSource = "github" | "npm_tarball";

export interface Config {
  settings: {
    output_dir: string;
    prune: boolean;
    max_file_size_kb: number;
    sync_concurrency: number;
    docs_source: DocsSource;
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
  const docsSourceRaw = settings.docs_source;
  const hasExplicitDocsSource = Object.prototype.hasOwnProperty.call(settings, "docs_source");
  if (hasExplicitDocsSource && docsSourceRaw !== "github" && docsSourceRaw !== "npm_tarball") {
    throw new AiDocsError(
      `settings.docs_source must be "github" or "npm_tarball", got: ${String(docsSourceRaw)}`,
      "INVALID_CONFIG"
    );
  }

  const docsSource = docsSourceRaw === "github" || docsSourceRaw === "npm_tarball" ? docsSourceRaw : undefined;
  const hasLegacyExperimental = Object.prototype.hasOwnProperty.call(settings, "experimental_npm_tarball");
  const legacyExperimental = Boolean(settings.experimental_npm_tarball ?? false);

  const rawMaxFileSizeKb = settings.max_file_size_kb;
  if (rawMaxFileSizeKb !== undefined && typeof rawMaxFileSizeKb !== "number") {
    throw new AiDocsError(
      `settings.max_file_size_kb must be a positive integer, got: ${String(rawMaxFileSizeKb)}`,
      "INVALID_CONFIG"
    );
  }

  const maxFileSizeKb = rawMaxFileSizeKb === undefined ? 512 : rawMaxFileSizeKb;
  if (!Number.isInteger(maxFileSizeKb) || maxFileSizeKb <= 0) {
    throw new AiDocsError(
      `settings.max_file_size_kb must be a positive integer, got: ${String(rawMaxFileSizeKb)}`,
      "INVALID_CONFIG"
    );
  }

  const rawSyncConcurrency = settings.sync_concurrency;
  if (rawSyncConcurrency !== undefined && typeof rawSyncConcurrency !== "number") {
    throw new AiDocsError(
      `settings.sync_concurrency must be a positive integer, got: ${String(rawSyncConcurrency)}`,
      "INVALID_CONFIG"
    );
  }

  const syncConcurrency = rawSyncConcurrency === undefined ? 8 : rawSyncConcurrency;
  if (!Number.isInteger(syncConcurrency) || syncConcurrency <= 0) {
    throw new AiDocsError(
      `settings.sync_concurrency must be a positive integer, got: ${String(rawSyncConcurrency)}`,
      "INVALID_CONFIG"
    );
  }

  return {
    settings: {
      output_dir: String(settings.output_dir ?? "docs/ai/vendor-docs/node"),
      prune: Boolean(settings.prune ?? true),
      max_file_size_kb: maxFileSizeKb,
      sync_concurrency: syncConcurrency,
      docs_source: docsSource ?? (hasLegacyExperimental ? (legacyExperimental ? "npm_tarball" : "github") : "npm_tarball"),
    },
    packages,
  };
}
