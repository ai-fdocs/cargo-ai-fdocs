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

function asRecord(value: unknown, errorMessage: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new AiDocsError(errorMessage, "INVALID_CONFIG");
  }

  return value as Record<string, unknown>;
}

function requireString(value: unknown, field: string): string {
  if (typeof value !== "string") {
    throw new AiDocsError(`${field} must be a string, got: ${String(value)}`, "INVALID_CONFIG");
  }

  return value;
}

function requireNonEmptyString(value: unknown, field: string): string {
  const stringValue = requireString(value, field);
  if (stringValue.trim().length === 0) {
    throw new AiDocsError(`${field} must be a non-empty string`, "INVALID_CONFIG");
  }

  return stringValue;
}

function validatePackageConfig(packageName: string, rawConfig: unknown): PackageConfig {
  const pkg = asRecord(rawConfig, `packages.${packageName} must be a table`);

  const repo = requireNonEmptyString(pkg.repo, `packages.${packageName}.repo`);
  const subpath = pkg.subpath;
  const files = pkg.files;
  const aiNotes = pkg.ai_notes;

  if (subpath !== undefined && typeof subpath !== "string") {
    throw new AiDocsError(
      `packages.${packageName}.subpath must be a string, got: ${String(subpath)}`,
      "INVALID_CONFIG"
    );
  }

  if (files !== undefined) {
    if (!Array.isArray(files)) {
      throw new AiDocsError(`packages.${packageName}.files must be an array of strings`, "INVALID_CONFIG");
    }

    for (let i = 0; i < files.length; i++) {
      if (typeof files[i] !== "string" || files[i].trim().length === 0) {
        throw new AiDocsError(
          `packages.${packageName}.files[${i}] must be a non-empty string, got: ${String(files[i])}`,
          "INVALID_CONFIG"
        );
      }
    }
  }

  if (aiNotes !== undefined && typeof aiNotes !== "string") {
    throw new AiDocsError(
      `packages.${packageName}.ai_notes must be a string, got: ${String(aiNotes)}`,
      "INVALID_CONFIG"
    );
  }

  return {
    repo,
    subpath,
    files: files as string[] | undefined,
    ai_notes: aiNotes as string | undefined,
  };
}

export function loadConfig(projectRoot: string): Config {
  const configPath = join(projectRoot, "ai-fdocs.toml");
  if (!existsSync(configPath)) {
    throw new AiDocsError("ai-fdocs.toml not found", "CONFIG_NOT_FOUND");
  }

  const raw = readFileSync(configPath, "utf-8");
  const data = parse(raw) as Record<string, unknown>;

  const settings = (data.settings as Record<string, unknown> | undefined) ?? {};
  const rawPackages = (data.packages as Record<string, unknown> | undefined) ?? {};
  const packages = asRecord(rawPackages, "packages must be a table");
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
  const legacyExperimentalRaw = settings.experimental_npm_tarball;
  if (hasLegacyExperimental && typeof legacyExperimentalRaw !== "boolean") {
    throw new AiDocsError(
      `settings.experimental_npm_tarball must be a boolean, got: ${String(legacyExperimentalRaw)}`,
      "INVALID_CONFIG"
    );
  }
  const legacyExperimental = legacyExperimentalRaw ?? false;

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

  const outputDirRaw = settings.output_dir;
  const outputDir = outputDirRaw === undefined ? "fdocs/node" : requireNonEmptyString(outputDirRaw, "settings.output_dir");

  const pruneRaw = settings.prune;
  if (pruneRaw !== undefined && typeof pruneRaw !== "boolean") {
    throw new AiDocsError(`settings.prune must be a boolean, got: ${String(pruneRaw)}`, "INVALID_CONFIG");
  }
  const prune = pruneRaw ?? true;

  const validatedPackages: Record<string, PackageConfig> = {};
  for (const [packageName, packageConfig] of Object.entries(packages)) {
    validatedPackages[packageName] = validatePackageConfig(packageName, packageConfig);
  }

  return {
    settings: {
      output_dir: outputDir,
      prune,
      max_file_size_kb: maxFileSizeKb,
      sync_concurrency: syncConcurrency,
      docs_source: docsSource ?? (hasLegacyExperimental ? (legacyExperimental ? "npm_tarball" : "github") : "npm_tarball"),
    },
    packages: validatedPackages,
  };
}
