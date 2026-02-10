import { join } from "node:path";
import chalk from "chalk";
import pLimit from "p-limit";
import { loadConfig } from "../config.js";
import { resolveVersions } from "../resolver.js";
import { GitHubClient, type FetchedFile } from "../fetcher.js";
import { computeConfigHash } from "../config-hash.js";
import { isCachedV2, savePackageFiles, prune, readCachedInfo, type SavedPackage } from "../storage.js";
import { generateIndex } from "../index.js";
import { generateSummary, type SummaryFile } from "../summary.js";

const MAX_CONCURRENT = 8;

interface SyncTaskResult {
  saved: SavedPackage | null;
  status: "synced" | "cached" | "skipped" | "error";
  message?: string;
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
  const entries = Object.entries(config.packages);
  const limit = pLimit(MAX_CONCURRENT);

  const tasks = entries.map(([name, pkgConfig]) =>
    limit(async (): Promise<SyncTaskResult> => {
    const version = lockVersions.get(name);
    if (!version) return { saved: null, status: "skipped", message: `'${name}': not in lockfile` };

    const configHash = computeConfigHash(pkgConfig);
    if (!force && isCachedV2(outputDir, name, version, configHash)) {
      return { saved: readCachedInfo(outputDir, name, version, pkgConfig), status: "cached" };
    }

    let resolved;
    try {
      resolved = await github.resolveRef(pkgConfig.repo, name, version);
    } catch (e) {
      return { saved: null, status: "error", message: `${name}@${version}: ${(e as Error).message}` };
    }

    let fetchedFiles: FetchedFile[];
    try {
      fetchedFiles = pkgConfig.files
        ? await github.fetchExplicitFiles(pkgConfig.repo, resolved.gitRef, pkgConfig.files)
        : await github.fetchDefaultFiles(pkgConfig.repo, resolved.gitRef, pkgConfig.subpath);
    } catch (e) {
      return { saved: null, status: "error", message: `${name}@${version}: ${(e as Error).message}` };
    }

    if (fetchedFiles.length === 0) return { saved: null, status: "skipped", message: `${name}@${version}: no files found` };

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
      saved: { name, version, gitRef: resolved.gitRef, isFallback: resolved.isFallback, files: savedNames, aiNotes: pkgConfig.ai_notes ?? "" },
      status: "synced",
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

  console.log(chalk.green(`\n✅ Sync complete: ${synced} synced, ${cached} cached, ${skipped} skipped, ${errors} errors.`));
}
