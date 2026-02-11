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

type SyncSource = "github" | "npm_tarball";

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

async function tryFetchFromNpmTarball(
  npmRegistry: NpmRegistryClient,
  name: string,
  version: string,
  subpath?: string,
  files?: string[]
): Promise<{ files: FetchedFile[] } | { error: { message: string; code?: string } }> {
  try {
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

export async function cmdSync(projectRoot: string, force: boolean, reportFormat: string = "text"): Promise<void> {
  if (reportFormat !== "text" && reportFormat !== "json") {
    throw new AiDocsError(`Unsupported --report-format value: ${reportFormat}`, "INVALID_FORMAT");
  }
  const jsonMode = reportFormat === "json";

  if (!jsonMode) console.log(chalk.blue(`Starting sync (v0.2)...${force ? " (force mode)" : ""}`));

  const config = loadConfig(projectRoot);
  const lockVersions = resolveVersions(projectRoot);
  const outputDir = join(projectRoot, config.settings.output_dir);

  if (config.settings.prune) {
    if (!jsonMode) console.log(chalk.gray("Pruning outdated docs..."));
    prune(outputDir, config, lockVersions);
  }

  const github = new GitHubClient();
  const npmRegistry = new NpmRegistryClient();
  const entries = Object.entries(config.packages);
  const limit = pLimit(config.settings.sync_concurrency);
  const selectedSource: SyncSource = config.settings.docs_source;

  const tasks = entries.map(([name, pkgConfig]) =>
    limit(async (): Promise<SyncTaskResult> => {
      const version = lockVersions.get(name);
      if (!version) return { saved: null, status: "skipped", source: selectedSource, message: `'${name}': not in lockfile` };

      const configHash = computeConfigHash(pkgConfig);
      if (!force && isCachedV2(outputDir, name, version, configHash)) {
        return {
          saved: readCachedInfo(outputDir, name, version, pkgConfig),
          status: "cached",
          source: selectedSource,
        };
      }

      let fetchedFiles: FetchedFile[] | null = null;
      let resolved = { gitRef: "npm-tarball", isFallback: false };
      let taskSource: SyncSource = selectedSource;
      let npmFallbackAttempted = false;
      let lastFallbackError: { message: string; code?: string } | null = null;

      if (selectedSource === "npm_tarball") {
        npmFallbackAttempted = true;
        const tarball = await tryFetchFromNpmTarball(npmRegistry, name, version, pkgConfig.subpath, pkgConfig.files);
        if ("error" in tarball) {
          return {
            saved: null,
            status: "error",
            source: selectedSource,
            message: `${name}@${version}: ${tarball.error.message}`,
            errorCode: tarball.error.code,
          };
        }
        fetchedFiles = tarball.files;
      } else {
        try {
          resolved = await github.resolveRef(pkgConfig.repo, name, version);
        } catch (e) {
          const primaryErr = toErrorInfo(e);
          npmFallbackAttempted = true;
          const tarball = await tryFetchFromNpmTarball(npmRegistry, name, version, pkgConfig.subpath, pkgConfig.files);
          if ("error" in tarball) {
            lastFallbackError = tarball.error;
            return {
              saved: null,
              status: "error",
              source: selectedSource,
              message: `${name}@${version}: GitHub resolve failed (${primaryErr.message}); npm fallback failed (${tarball.error.message})`,
              errorCode: primaryErr.code ?? tarball.error.code,
            };
          }

          taskSource = "npm_tarball";
          fetchedFiles = tarball.files;
          resolved = { gitRef: "npm-tarball", isFallback: false };
        }

        if (!fetchedFiles) {
          try {
            fetchedFiles = pkgConfig.files
              ? await github.fetchExplicitFiles(pkgConfig.repo, resolved.gitRef, pkgConfig.files)
              : await github.fetchDefaultFiles(pkgConfig.repo, resolved.gitRef, pkgConfig.subpath);
          } catch (e) {
            const primaryErr = toErrorInfo(e);
            npmFallbackAttempted = true;
            const tarball = await tryFetchFromNpmTarball(npmRegistry, name, version, pkgConfig.subpath, pkgConfig.files);
            if ("error" in tarball) {
              lastFallbackError = tarball.error;
              return {
                saved: null,
                status: "error",
                source: selectedSource,
                message: `${name}@${version}: GitHub fetch failed (${primaryErr.message}); npm fallback failed (${tarball.error.message})`,
                errorCode: primaryErr.code ?? tarball.error.code,
              };
            }

            taskSource = "npm_tarball";
            fetchedFiles = tarball.files;
            resolved = { gitRef: "npm-tarball", isFallback: false };
          }
        }
      }

      if (!fetchedFiles || fetchedFiles.length === 0) {
        let emptyFallbackError: { message: string; code?: string } | null = null;

        if (selectedSource === "github" && !npmFallbackAttempted) {
          npmFallbackAttempted = true;
          const tarball = await tryFetchFromNpmTarball(npmRegistry, name, version, pkgConfig.subpath, pkgConfig.files);
          if (!("error" in tarball) && tarball.files.length > 0) {
            fetchedFiles = tarball.files;
            taskSource = "npm_tarball";
            resolved = { gitRef: "npm-tarball", isFallback: false };
          } else if ("error" in tarball) {
            emptyFallbackError = tarball.error;
            lastFallbackError = tarball.error;
          }
        }

        if (!fetchedFiles || fetchedFiles.length === 0) {
          const fallbackError = emptyFallbackError ?? lastFallbackError;
          const details = fallbackError ? `; npm fallback failed (${fallbackError.message})` : "";
          return { saved: null, status: "skipped", source: taskSource, message: `${name}@${version}: no files found${details}` };
        }
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
      const files: SummaryFile[] = fetchedFiles.map((f, i) => ({
        flatName: savedNames[i],
        originalPath: f.path,
        sizeBytes: Buffer.byteLength(f.content, "utf-8"),
      }));

      generateSummary(pkgDir, {
        packageName: name,
        version,
        repo: pkgConfig.repo,
        gitRef: resolved.gitRef,
        isFallback: resolved.isFallback,
        aiNotes: pkgConfig.ai_notes ?? "",
        files,
      });

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
  for (const result of results) {
    if (result.saved) savedPackages.push(result.saved);
    if (!jsonMode && result.status === "skipped") {
      console.log(chalk.yellow(`  ⏭ ${result.message}`));
    }
    if (!jsonMode && result.status === "error") {
      console.log(chalk.red(`  ❌ ${result.message}`));
    }
  }

  savedPackages.sort((a, b) => a.name.localeCompare(b.name));
  generateIndex(outputDir, savedPackages);

  const report = buildSyncReport(results, selectedSource);

  if (reportFormat === "json") {
    console.log(JSON.stringify(report, null, 2));
  } else {
    printSyncSummary(report);
  }
}
