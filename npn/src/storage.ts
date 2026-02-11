import {
  existsSync,
  readFileSync,
  writeFileSync,
  mkdirSync,
  rmSync,
  readdirSync,
  statSync,
} from "node:fs";
import { join, extname } from "node:path";
import chalk from "chalk";
import { truncateChangelog } from "./changelog.js";
import type { Config, PackageConfig } from "./config.js";
import type { ResolvedRef, FetchedFile } from "./fetcher.js";

export interface CrateMeta {
  schema_version: number;
  version: string;
  git_ref: string;
  fetched_at: string;
  is_fallback: boolean;
  config_hash?: string;
}

export interface SavedPackage {
  name: string;
  version: string;
  gitRef: string;
  isFallback: boolean;
  files: string[];
  aiNotes: string;
}

export function flattenFilename(filePath: string): string {
  return filePath.includes("/") ? filePath.replace(/\//g, "__") : filePath;
}

function shouldInjectHeader(filePath: string): boolean {
  const ext = extname(filePath).toLowerCase();
  return ext === ".md" || ext === ".html" || ext === ".htm";
}

function injectHeader(
  content: string,
  ownerRepo: string,
  gitRef: string,
  originalPath: string,
  isFallback: boolean,
  version: string
): string {
  const date = new Date().toISOString().split("T")[0];
  let header = `<!-- AI-FDOCS: source=github.com/${ownerRepo} ref=${gitRef} path=${originalPath} fetched=${date} -->\n`;

  if (isFallback) {
    header += `<!-- AI-FDOCS WARNING: No tag found for version ${version}. Fetched from '${gitRef}' branch. -->\n`;
  }

  return header + content;
}

function truncateIfNeeded(content: string, maxSizeKb: number): string {
  const maxBytes = maxSizeKb * 1024;
  if (Buffer.byteLength(content, "utf-8") <= maxBytes) return content;
  const truncated = Buffer.from(content, "utf-8").subarray(0, maxBytes).toString("utf-8");
  return `${truncated}\n\n[TRUNCATED by ai-fdocs at ${maxSizeKb}KB]\n`;
}

export function isCachedV2(outputDir: string, packageName: string, version: string, configHash: string): boolean {
  const pkgDir = join(outputDir, `${packageName}@${version}`);
  const metaPath = join(pkgDir, ".aifd-meta.toml");
  if (!existsSync(metaPath)) return false;

  try {
    const meta = parseMetaToml(readFileSync(metaPath, "utf-8"));
    if (meta.version !== version) return false;
    if (meta.config_hash) return meta.config_hash === configHash;
    return true;
  } catch {
    return false;
  }
}

function parseMetaToml(raw: string): CrateMeta {
  const get = (key: string): string | undefined => raw.match(new RegExp(`${key}\\s*=\\s*"([^"]*)"`))?.[1];
  const getBool = (key: string): boolean => raw.match(new RegExp(`${key}\\s*=\\s*(true|false)`))?.[1] === "true";

  const schemaVersionRaw = raw.match(/schema_version\s*=\s*(\d+)/)?.[1];

  return {
    schema_version: schemaVersionRaw ? Number(schemaVersionRaw) : 1,
    version: get("version") ?? "",
    git_ref: get("git_ref") ?? "",
    fetched_at: get("fetched_at") ?? "",
    is_fallback: getBool("is_fallback"),
    config_hash: get("config_hash"),
  };
}

export function savePackageFiles(
  outputDir: string,
  packageName: string,
  version: string,
  resolved: ResolvedRef,
  fetchedFiles: FetchedFile[],
  pkgConfig: PackageConfig,
  maxFileSizeKb: number,
  configHash: string
): string[] {
  const pkgDir = join(outputDir, `${packageName}@${version}`);
  if (existsSync(pkgDir)) rmSync(pkgDir, { recursive: true, force: true });
  mkdirSync(pkgDir, { recursive: true });

  const savedNames: string[] = [];
  for (const file of fetchedFiles) {
    const flatName = flattenFilename(file.path);
    let content = file.content;

    if (file.path.toLowerCase().includes("changelog")) content = truncateChangelog(content, version);
    content = truncateIfNeeded(content, maxFileSizeKb);

    if (shouldInjectHeader(file.path)) {
      content = injectHeader(content, pkgConfig.repo, resolved.gitRef, file.path, resolved.isFallback, version);
    }

    writeFileSync(join(pkgDir, flatName), content, "utf-8");
    savedNames.push(flatName);
  }

  const date = new Date().toISOString().split("T")[0];
  const meta = [
    "schema_version = 2",
    `version = "${version}"`,
    `git_ref = "${resolved.gitRef}"`,
    `fetched_at = "${date}"`,
    `is_fallback = ${resolved.isFallback}`,
    `config_hash = "${configHash}"`,
  ].join("\n") + "\n";

  writeFileSync(join(pkgDir, ".aifd-meta.toml"), meta, "utf-8");
  console.log(chalk.green(`  💾 ${packageName}@${version}: ${savedNames.length} files saved.`));
  return savedNames;
}

export function prune(outputDir: string, config: Config, lockVersions: Map<string, string>): void {
  if (!existsSync(outputDir)) return;

  for (const entry of readdirSync(outputDir)) {
    const fullPath = join(outputDir, entry);
    if (!statSync(fullPath).isDirectory()) continue;

    const at = entry.lastIndexOf("@");
    if (at <= 0) continue;

    const dirPkg = entry.slice(0, at);
    const dirVersion = entry.slice(at + 1);

    let remove = false;
    if (!config.packages[dirPkg]) {
      remove = true;
      console.log(chalk.gray(`  🗑 Pruning ${entry}: removed from config.`));
    } else {
      const lockVer = lockVersions.get(dirPkg);
      if (!lockVer || lockVer !== dirVersion) {
        remove = true;
        console.log(chalk.gray(`  🗑 Pruning ${entry}: lockfile mismatch.`));
      }
    }

    if (remove) rmSync(fullPath, { recursive: true, force: true });
  }
}

export function readCachedInfo(
  outputDir: string,
  packageName: string,
  version: string,
  pkgConfig: PackageConfig
): SavedPackage | null {
  const pkgDir = join(outputDir, `${packageName}@${version}`);
  const metaPath = join(pkgDir, ".aifd-meta.toml");
  if (!existsSync(metaPath)) return null;

  try {
    const meta = parseMetaToml(readFileSync(metaPath, "utf-8"));
    const files = readdirSync(pkgDir).filter((f) => !f.startsWith(".") && f !== "_SUMMARY.md");

    return {
      name: packageName,
      version,
      gitRef: meta.git_ref,
      isFallback: meta.is_fallback,
      files,
      aiNotes: pkgConfig.ai_notes ?? "",
    };
  } catch {
    return null;
  }
}
