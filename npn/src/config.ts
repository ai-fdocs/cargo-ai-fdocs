import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { parse } from "smol-toml";
import { AiDocsError } from "./error.js";

export type DocsSource = "github" | "npm_tarball";
export type SyncMode = "lockfile" | "latest_docs" | "hybrid";

export interface Settings {
  output_dir: string;
  max_file_size_kb: number;
  prune: boolean;
  sync_concurrency: number;
  docs_source: DocsSource;
  sync_mode: SyncMode;
  latest_ttl_hours: number;
}

export interface PackageConfig {
  repo?: string;
  subpath?: string;
  files?: string[];
  ai_notes?: string;
}

export interface Config {
  settings: Settings;
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

function requireNumber(value: unknown, field: string): number {
  if (typeof value !== "number") {
    throw new AiDocsError(`${field} must be a number, got: ${String(value)}`, "INVALID_CONFIG");
  }
  return value;
}

function requireInteger(value: unknown, field: string): number {
  const num = requireNumber(value, field);
  if (!Number.isInteger(num)) {
    throw new AiDocsError(`${field} must be an integer`, "INVALID_CONFIG");
  }
  return num;
}

function requireBoolean(value: unknown, field: string): boolean {
  if (typeof value !== "boolean") {
    throw new AiDocsError(`${field} must be a boolean`, "INVALID_CONFIG");
  }
  return value;
}

function validatePackageConfig(packageName: string, rawConfig: unknown): PackageConfig {
  const pkg = asRecord(rawConfig, `packages.${packageName} must be a table`);

  const repo = pkg.repo;
  const subpath = pkg.subpath;
  const files = pkg.files;
  const aiNotes = pkg.ai_notes;

  if (repo !== undefined) {
    if (typeof repo !== "string" || repo.trim().length === 0) {
      throw new AiDocsError(`packages.${packageName}.repo must be a non-empty string`, "INVALID_CONFIG");
    }
  }

  if (subpath !== undefined) {
    if (typeof subpath !== "string" || subpath.trim().length === 0) {
      throw new AiDocsError(
        `packages.${packageName}.subpath must be a non-empty string`,
        "INVALID_CONFIG"
      );
    }
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
    repo: repo as string | undefined,
    subpath,
    files: files as string[] | undefined,
    ai_notes: aiNotes as string | undefined,
  };
}

export function loadConfig(projectRoot: string): Config {
  const configPath = join(projectRoot, "ai-fdocs.toml");
  if (!existsSync(configPath)) {
    throw new AiDocsError(`Config not found: ${configPath}`, "FILE_NOT_FOUND");
  }

  const content = readFileSync(configPath, "utf-8");
  const raw = parse(content) as Record<string, any>;

  const settingsRaw = asRecord(raw.settings || {}, "settings must be a table");
  const packagesRaw = asRecord(raw.packages || {}, "packages must be a table");

  const docsSourceRaw = settingsRaw.docs_source;
  let docsSource: DocsSource = "npm_tarball";
  if (docsSourceRaw !== undefined) {
    const s = requireString(docsSourceRaw, "settings.docs_source");
    if (s === "github" || s === "npm_tarball") {
      docsSource = s;
    } else {
      throw new AiDocsError('settings.docs_source must be "github" or "npm_tarball"', "INVALID_CONFIG");
    }
  } else if (settingsRaw.experimental_npm_tarball === true) {
    docsSource = "npm_tarball";
  } else if (settingsRaw.experimental_npm_tarball === false) {
    docsSource = "github";
  }

  const syncModeRaw = settingsRaw.sync_mode;
  let syncMode: SyncMode = "lockfile";
  if (syncModeRaw === "lockfile" || syncModeRaw === "latest_docs" || syncModeRaw === "hybrid") {
    syncMode = syncModeRaw;
  } else if (syncModeRaw === "latest-docs") {
    syncMode = "latest_docs";
  }

  const config: Config = {
    settings: {
      output_dir: settingsRaw.output_dir !== undefined ? requireNonEmptyString(settingsRaw.output_dir, "settings.output_dir") : "fdocs/node",
      max_file_size_kb: settingsRaw.max_file_size_kb !== undefined ? requireInteger(settingsRaw.max_file_size_kb, "settings.max_file_size_kb") : 512,
      prune: settingsRaw.prune !== undefined ? requireBoolean(settingsRaw.prune, "settings.prune") : true,
      sync_concurrency: settingsRaw.sync_concurrency !== undefined ? requireInteger(settingsRaw.sync_concurrency, "settings.sync_concurrency") : 8,
      docs_source: docsSource,
      sync_mode: syncMode,
      latest_ttl_hours: settingsRaw.latest_ttl_hours !== undefined ? requireInteger(settingsRaw.latest_ttl_hours, "settings.latest_ttl_hours") : 24,
    },
    packages: {},
  };

  for (const [name, pkgRaw] of Object.entries(packagesRaw)) {
    config.packages[name] = validatePackageConfig(name, pkgRaw);
  }

  validateConfig(config);
  return config;
}

function validateConfig(config: Config): void {
  const { settings, packages } = config;

  if (settings.sync_concurrency <= 0 || settings.sync_concurrency > 50) {
    throw new AiDocsError("settings.sync_concurrency must be between 1 and 50", "INVALID_CONFIG");
  }

  if (settings.max_file_size_kb <= 0) {
    throw new AiDocsError("settings.max_file_size_kb must be greater than 0", "INVALID_CONFIG");
  }

  if (settings.latest_ttl_hours <= 0) {
    throw new AiDocsError("settings.latest_ttl_hours must be greater than 0", "INVALID_CONFIG");
  }

  const isLockfileMode = settings.sync_mode === "lockfile";
  const isHybridMode = settings.sync_mode === "hybrid";
  const isGithubSource = settings.docs_source === "github";

  for (const [name, pkg] of Object.entries(packages)) {
    if ((isGithubSource || isHybridMode) && !pkg.repo) {
      throw new AiDocsError(`Package '${name}' must define 'repo' for github source or hybrid mode`, "INVALID_CONFIG");
    }

    if (isLockfileMode && isGithubSource && !pkg.repo) {
      throw new AiDocsError(`Package '${name}' must define 'repo' for github source in lockfile mode`, "INVALID_CONFIG");
    }
  }
}
