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

const MAX_CONCURRENT = 8;

type SyncSource = "github" | "npm_tarball";

interface SyncTaskResult {
  saved: SavedPackage | null;
  status: "synced" | "cached" | "skipped" | "error";
  source: SyncSource;
  message?: string;
}

interface SourceStat {
  synced: number;
  errors: number;
  skipped: number;
  cached: number;
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

export async function cmdSync(projectRoot: string, force: boolean): Promise<void> {
  console.log(chalk.blue(`Starting sync (v0.2)...${force ? " (force mode)" : ""}`));

  const config = loadConfig(projectRoot);
  const lockVersions = resolveVersions(projectRoot);
  const outputDir = join(projectRoot, config.settings.output_dir);

  if (config.settings.prune) {
    console.log(chalk.gray("Pruning outdated docs..."));
    prune(outputDir, config, lockVersions);
  }

  const github = new GitHubClient();
  const npmRegistry = new NpmRegistryClient();
  const entries = Object.entries(config.packages);
  const limit = pLimit(MAX_CONCURRENT);
  const selectedSource: SyncSource = config.settings.experimental_npm_tarball ? "npm_tarball" : "github";

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

      let fetchedFiles: FetchedFile[];
      let resolved = { gitRef: "npm-tarball", isFallback: false };

      if (selectedSource === "npm_tarball") {
        try {
          const tarballUrl = await npmRegistry.getTarballUrl(name, version);
          if (!tarballUrl) {
            return { saved: null, status: "error", source: selectedSource, message: `${name}@${version}: no npm tarball URL` };
          }

          fetchedFiles = await fetchDocsFromNpmTarball(tarballUrl, pkgConfig.subpath, pkgConfig.files);
        } catch (e) {
          return { saved: null, status: "error", source: selectedSource, message: `${name}@${version}: ${(e as Error).message}` };
        }
      } else {
        try {
          resolved = await github.resolveRef(pkgConfig.repo, name, version);
        } catch (e) {
          return { saved: null, status: "error", source: selectedSource, message: `${name}@${version}: ${(e as Error).message}` };
        }

        try {
          fetchedFiles = pkgConfig.files
            ? await github.fetchExplicitFiles(pkgConfig.repo, resolved.gitRef, pkgConfig.files)
            : await github.fetchDefaultFiles(pkgConfig.repo, resolved.gitRef, pkgConfig.subpath);
        } catch (e) {
          return { saved: null, status: "error", source: selectedSource, message: `${name}@${version}: ${(e as Error).message}` };
        }
      }

      if (fetchedFiles.length === 0) {
        return { saved: null, status: "skipped", source: selectedSource, message: `${name}@${version}: no files found` };
      }

      const savedNames = savePackageFiles(
        outputDir,
        name,
        version,
        resolved,
        fetchedFiles,
        pkgConfig,
        config.settings.max_file_size_kb,
        configHash
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
        source: selectedSource,
      };
    })
  );

  const results = await Promise.all(tasks);

  const savedPackages: SavedPackage[] = [];
  let synced = 0;
  let cached = 0;
  let skipped = 0;
  let errors = 0;

  for (const result of results) {
    if (result.saved) savedPackages.push(result.saved);
    if (result.status === "synced") synced++;
    if (result.status === "cached") cached++;
    if (result.status === "skipped") {
      skipped++;
      console.log(chalk.yellow(`  ⏭ ${result.message}`));
    }
    if (result.status === "error") {
      errors++;
      console.log(chalk.red(`  ❌ ${result.message}`));
    }
  }

  savedPackages.sort((a, b) => a.name.localeCompare(b.name));
  generateIndex(outputDir, savedPackages);

  const sourceStats = summarizeSourceStats(results);
  const activeStats = sourceStats[selectedSource];
  console.log(
    chalk.gray(
      `Source stats (${selectedSource}): synced=${activeStats.synced}, cached=${activeStats.cached}, skipped=${activeStats.skipped}, errors=${activeStats.errors}`
    )
  );

  console.log(chalk.green(`\n✅ Sync complete: ${synced} synced, ${cached} cached, ${skipped} skipped, ${errors} errors.`));
}
