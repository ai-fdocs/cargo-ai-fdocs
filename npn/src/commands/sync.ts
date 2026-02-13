import { join } from "node:path";
import chalk from "chalk";
import pLimit from "p-limit";
import { loadConfig } from "../config.js";
import { resolveVersions } from "../resolver.js";
import { GitHubClient, fetchDocsFromNpmTarball, type FetchedFile } from "../fetcher.js";
import { computeConfigHash } from "../config-hash.js";
import { isCachedV2, savePackageFiles, prune, readCachedInfo, type SavedPackage } from "../storage.js";
import { generateIndex } from "../index.js";
import { generateSummary, type SummaryFile } from "../summary.js";
import { NpmRegistryClient } from "../registry.js";
import { AiDocsError } from "../error.js";
import { generateConsolidatedDoc } from "../consolidation.js";

type SyncSource = "github" | "npm_tarball" | "hybrid";

interface SyncTaskResult {
  saved: SavedPackage | null;
  status: "synced" | "cached" | "skipped" | "error";
  source: SyncSource;
  message?: string;
  errorCode?: string;
}

interface SourceStat {
  synced: number;
  errors: number;
  skipped: number;
  cached: number;
}

export interface SyncReport {
  source: SyncSource;
  totals: {
    synced: number;
    cached: number;
    skipped: number;
    errors: number;
  };
  sourceStats: Record<SyncSource, SourceStat>;
  errorCodes: Record<string, number>;
  issues: string[];
}

export function summarizeSourceStats(results: SyncTaskResult[]): Record<SyncSource, SourceStat> {
  const stats: Record<SyncSource, SourceStat> = {
    github: { synced: 0, errors: 0, skipped: 0, cached: 0 },
    npm_tarball: { synced: 0, errors: 0, skipped: 0, cached: 0 },
    hybrid: { synced: 0, errors: 0, skipped: 0, cached: 0 },
  };

  for (const result of results) {
    if (result.status === "error") stats[result.source].errors++;
    else stats[result.source][result.status]++;
  }

  return stats;
}

export function summarizeErrorCodes(results: SyncTaskResult[]): Record<string, number> {
  const counts: Record<string, number> = {};

  for (const result of results) {
    if (result.status !== "error") continue;
    const code = result.errorCode ?? "UNKNOWN";
    counts[code] = (counts[code] ?? 0) + 1;
  }

  return counts;
}

export function buildSyncReport(results: SyncTaskResult[], source: SyncSource): SyncReport {
  return {
    source,
    totals: {
      synced: results.filter((r) => r.status === "synced").length,
      cached: results.filter((r) => r.status === "cached").length,
      skipped: results.filter((r) => r.status === "skipped").length,
      errors: results.filter((r) => r.status === "error").length,
    },
    sourceStats: summarizeSourceStats(results),
    errorCodes: summarizeErrorCodes(results),
    issues: results
      .filter((r) => r.status === "error" || r.status === "skipped")
      .map((r) => r.message)
      .filter((msg): msg is string => Boolean(msg && msg.length > 0)),
  };
}

function toErrorInfo(error: unknown): { message: string; code?: string } {
  if (error instanceof AiDocsError) {
    return { message: error.message, code: error.code };
  }
  if (error instanceof Error) {
    return { message: error.message };
  }
  return { message: String(error) };
}

function printSyncSummary(report: SyncReport): void {
  const activeStats = report.sourceStats[report.source];
  console.log(
    chalk.gray(
      `Source stats (${report.source}): synced=${activeStats.synced}, cached=${activeStats.cached}, skipped=${activeStats.skipped}, errors=${activeStats.errors}`
    )
  );

  const errorCodeSummary = Object.entries(report.errorCodes)
    .sort((a, b) => b[1] - a[1])
    .map(([code, count]) => `${code}=${count}`)
    .join(", ");
  if (errorCodeSummary) {
    console.log(chalk.gray(`Error summary: ${errorCodeSummary}`));
  }

  console.log(
    chalk.green(
      `\n✅ Sync complete: ${report.totals.synced} synced, ${report.totals.cached} cached, ${report.totals.skipped} skipped, ${report.totals.errors} errors.`
    )
  );
}

async function tryFetchFromNpm(
  npmRegistry: NpmRegistryClient,
  name: string,
  version: string,
  subpath?: string,
  files?: string[]
): Promise<{ files: FetchedFile[] } | { error: { message: string; code?: string } }> {
  try {
    // Fast path: try to get README from registry metadata if no explicit files or only README is requested
    const isReadmeOnly = !files || (files.length === 1 && files[0].toLowerCase() === "readme.md");
    if (isReadmeOnly && !subpath) {
      const readme = await npmRegistry.getReadme(name, version);
      if (readme) {
        return { files: [{ path: "README.md", content: readme }] };
      }
    }

    const tarballUrl = await npmRegistry.getTarballUrl(name, version);
    if (!tarballUrl) {
      return {
        error: {
          message: "no npm tarball URL",
          code: "NPM_TARBALL_NOT_FOUND",
        },
      };
    }

    const fetched = await fetchDocsFromNpmTarball(tarballUrl, subpath, files);
    return { files: fetched };
  } catch (e) {
    const err = toErrorInfo(e);
    return { error: { message: err.message, code: err.code } };
  }
}

export async function cmdSync(projectRoot: string, options: { force?: boolean; mode?: string; reportFormat?: string }): Promise<void> {
  const force = options.force || false;
  const reportFormat = options.reportFormat || "text";

  if (reportFormat !== "text" && reportFormat !== "json") {
    throw new AiDocsError(`Unsupported --report-format value: ${reportFormat}`, "INVALID_FORMAT");
  }
  const jsonMode = reportFormat === "json";

  const config = loadConfig(projectRoot);
  const syncMode = options.mode || config.settings.sync_mode;

  if (!jsonMode) {
    console.log(chalk.blue(`Starting sync (mode: ${syncMode})...${force ? " (force mode)" : ""}`));
  }

  const outputDir = join(projectRoot, config.settings.output_dir);
  const npmRegistry = new NpmRegistryClient();
  const github = new GitHubClient();

  let targetVersions: Map<string, string>;
  if (syncMode === "latest_docs") {
    targetVersions = new Map();
    if (!jsonMode) console.log(chalk.gray("Resolving latest versions from npm registry..."));
    for (const name of Object.keys(config.packages)) {
      try {
        const ver = await npmRegistry.getLatestVersion(name);
        targetVersions.set(name, ver);
      } catch (e) {
        if (!jsonMode) console.log(chalk.yellow(`  ⚠️ Failed to resolve latest version for ${name}: ${String(e)}`));
      }
    }
  } else {
    targetVersions = resolveVersions(projectRoot);
  }

  if (config.settings.prune) {
    if (!jsonMode) console.log(chalk.gray("Pruning outdated docs..."));
    prune(outputDir, config, targetVersions);
  }

  const entries = Object.entries(config.packages);
  const limit = pLimit(config.settings.sync_concurrency);
  const selectedSource: SyncSource = config.settings.docs_source;

  const tasks = entries.map(([name, pkgConfig]) =>
    limit(async (): Promise<SyncTaskResult> => {
      const version = targetVersions.get(name);
      if (!version) {
        return {
          saved: null,
          status: "skipped",
          source: selectedSource,
          message: `'${name}': version resolution failed`,
        };
      }

      const configHash = computeConfigHash(pkgConfig);
      const isLatestMode = syncMode === "latest_docs";

      if (!force && isCachedV2(outputDir, name, version, configHash)) {
        const cached = readCachedInfo(outputDir, name, version, pkgConfig);
        if (isLatestMode) {
          const fetchedAt = (cached as any).fetchedAt; // Storage.ts adds this
          if (fetchedAt && isLatestCacheFreshSync(fetchedAt, config.settings.latest_ttl_hours)) {
            return { saved: cached, status: "cached", source: selectedSource };
          }
          if (!jsonMode) console.log(chalk.gray(`  🔄 ${name}@${version}: cache expired, refreshing...`));
        } else {
          return { saved: cached, status: "cached", source: selectedSource };
        }
      }

      let fetchedFiles: FetchedFile[] | null = null;
      let resolved = { gitRef: isLatestMode ? "npm-tarball" : "lockfile", isFallback: false };
      let taskSource: SyncSource = selectedSource;
      const isHybrid = syncMode === "hybrid";

      if (isHybrid) {
        // Hybrid mode: GitHub for Meta, NPM for Docs
        const repo = pkgConfig.repo;
        if (!repo) {
          return {
            saved: null,
            status: "error",
            source: selectedSource,
            message: `${name}@${version}: missing repo for hybrid mode`,
            errorCode: "CONFIG_ERROR",
          };
        }

        try {
          // 1. Resolve GitHub for Metadata (Changelog)
          resolved = await github.resolveRef(repo, name, version);
          const metaFiles = await github.fetchExplicitFiles(repo, resolved.gitRef, ["CHANGELOG.md", "CHANGES.md", "HISTORY.md"]);

          // 2. Fetch Docs from NPM
          const npmDocs = await tryFetchFromNpm(npmRegistry, name, version, pkgConfig.subpath, pkgConfig.files);

          if ("error" in npmDocs) {
            // If NPM fails, we fallback to GitHub entirely for docs too? 
            // For hybrid MVP, lets try to get README at least from GitHub if NPM fails.
            const githubDocs = await github.fetchDefaultFiles(repo, resolved.gitRef, pkgConfig.subpath);
            fetchedFiles = [...metaFiles, ...githubDocs];
            taskSource = "github";
          } else {
            fetchedFiles = [...metaFiles, ...npmDocs.files];
            taskSource = "npm_tarball";
          }
        } catch (e) {
          const err = toErrorInfo(e);
          return {
            saved: null,
            status: "error",
            source: selectedSource,
            message: `${name}@${version}: Hybrid fetch failed: ${err.message}`,
            errorCode: err.code,
          };
        }
      } else if (selectedSource === "npm_tarball") {
        const npmDocs = await tryFetchFromNpm(npmRegistry, name, version, pkgConfig.subpath, pkgConfig.files);
        if ("error" in npmDocs) {
          return {
            saved: null,
            status: "error",
            source: selectedSource,
            message: `${name}@${version}: ${npmDocs.error.message}`,
            errorCode: npmDocs.error.code,
          };
        }
        fetchedFiles = npmDocs.files;
      } else {
        // Source is GitHub
        const repo = pkgConfig.repo;
        if (!repo) {
          return {
            saved: null,
            status: "error",
            source: selectedSource,
            message: `${name}@${version}: missing repo for github source`,
            errorCode: "CONFIG_ERROR",
          };
        }

        try {
          resolved = await github.resolveRef(repo, name, version);
          fetchedFiles = pkgConfig.files
            ? await github.fetchExplicitFiles(repo, resolved.gitRef, pkgConfig.files)
            : await github.fetchDefaultFiles(repo, resolved.gitRef, pkgConfig.subpath);
        } catch (e) {
          const primaryErr = toErrorInfo(e);
          // Try npm fallback
          const npmFallback = await tryFetchFromNpm(npmRegistry, name, version, pkgConfig.subpath, pkgConfig.files);
          if ("error" in npmFallback) {
            return {
              saved: null,
              status: "error",
              source: selectedSource,
              message: `${name}@${version}: GitHub failed (${primaryErr.message}); npm fallback failed (${npmFallback.error.message})`,
              errorCode: primaryErr.code || npmFallback.error.code,
            };
          }
          taskSource = "npm_tarball";
          fetchedFiles = npmFallback.files;
          resolved = { gitRef: "npm-tarball", isFallback: true };
        }
      }

      if (!fetchedFiles || fetchedFiles.length === 0) {
        return {
          saved: null,
          status: "skipped",
          source: taskSource,
          message: `${name}@${version}: no docs found`,
        };
      }

      const savedNames = savePackageFiles(
        outputDir,
        name,
        version,
        resolved,
        fetchedFiles,
        pkgConfig,
        config.settings.max_file_size_kb,
        configHash,
        jsonMode
      );

      const pkgDir = join(outputDir, `${name}@${version}`);
      const summaryFiles: SummaryFile[] = fetchedFiles.map((f, i) => ({
        flatName: savedNames[i],
        originalPath: f.path,
        sizeBytes: Buffer.byteLength(f.content, "utf-8"),
      }));

      generateSummary(pkgDir, {
        packageName: name,
        version,
        repo: pkgConfig.repo || "",
        gitRef: resolved.gitRef,
        isFallback: resolved.isFallback,
        aiNotes: pkgConfig.ai_notes ?? "",
        files: summaryFiles,
      });

      // Generate AI-optimized consolidated doc
      generateConsolidatedDoc(
        pkgDir,
        {
          packageName: name,
          version,
          repo: pkgConfig.repo || "",
          gitRef: resolved.gitRef,
          isFallback: resolved.isFallback,
          aiNotes: pkgConfig.ai_notes ?? "",
          files: summaryFiles,
        },
        { includeChangelog: true, normalizeMarkdown: true }
      );

      return {
        saved: {
          name,
          version,
          gitRef: resolved.gitRef,
          isFallback: resolved.isFallback,
          files: savedNames,
          aiNotes: pkgConfig.ai_notes ?? "",
        },
        status: "synced",
        source: taskSource,
      };
    })
  );

  const results = await Promise.all(tasks);
  const savedPackages: SavedPackage[] = [];
  for (const r of results) {
    if (r.saved) savedPackages.push(r.saved);
    if (!jsonMode && r.status === "skipped") console.log(chalk.yellow(`  ⏭ ${r.message}`));
    if (!jsonMode && r.status === "error") console.log(chalk.red(`  ❌ ${r.message}`));
  }

  savedPackages.sort((a, b) => a.name.localeCompare(b.name));
  generateIndex(outputDir, savedPackages);

  const report = buildSyncReport(results, selectedSource);
  if (jsonMode) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    printSyncSummary(report);
  }
}

function isLatestCacheFreshSync(fetchedAt: string, ttlHours: number): boolean {
  try {
    const date = new Date(fetchedAt);
    if (isNaN(date.getTime())) return false;
    return Date.now() - date.getTime() < ttlHours * 60 * 60 * 1000;
  } catch {
    return false;
  }
}
