import { join } from "node:path";
import { existsSync, readFileSync } from "node:fs";
import chalk from "chalk";
import { loadConfig, type SyncMode } from "../config.js";
import { resolveVersions } from "../resolver.js";
import { computeConfigHash } from "../config-hash.js";
import { NpmRegistryClient } from "../registry.js";

export type DocsStatus = "Synced" | "SyncedFallback" | "Outdated" | "Missing" | "Incomplete" | "ReadError";

export interface PackageStatus {
  name: string;
  lockVersion: string | null;
  status: DocsStatus;
  reason: string;
  configOk: boolean;
  isFallback: boolean;
}

export interface StatusReport {
  summary: {
    total: number;
    synced: number;
    problems: number;
  };
  packages: PackageStatus[];
}

export async function cmdStatus(
  projectRoot: string,
  format: string = "text",
  modeOverride?: string
): Promise<void> {
  const config = loadConfig(projectRoot);
  const syncMode = (modeOverride as SyncMode) || config.settings.sync_mode;
  const outputDir = join(projectRoot, config.settings.output_dir);

  let targetVersions: Map<string, string>;
  const registry = new NpmRegistryClient();

  if (syncMode === "latest_docs") {
    targetVersions = new Map();
    for (const name of Object.keys(config.packages)) {
      try {
        const ver = await registry.getLatestVersion(name);
        targetVersions.set(name, ver);
      } catch {
        // Fallback to lockfile if registry fails? Or just skip?
        // Match Rust: warn but proceed
      }
    }
  } else {
    targetVersions = resolveVersions(projectRoot);
  }

  const statuses: PackageStatus[] = [];

  for (const [name, pkgConfig] of Object.entries(config.packages)) {
    const targetVersion = targetVersions.get(name) ?? null;
    let status: DocsStatus = "Synced";
    let reason = "up to date";
    let configOk = true;
    let isFallback = false;

    if (!targetVersion) {
      status = "Missing";
      reason = syncMode === "latest_docs" ? "Registry resolve failed" : "Not in lockfile";
    } else {
      const pkgDir = join(outputDir, `${name}@${targetVersion}`);
      const metaPath = join(pkgDir, ".aifd-meta.toml");

      if (!existsSync(pkgDir)) {
        status = "Missing";
        reason = "Missing artifacts";
      } else if (!existsSync(metaPath)) {
        status = "Incomplete";
        reason = "Missing metadata";
      } else {
        try {
          const raw = readFileSync(metaPath, "utf-8");
          isFallback = raw.match(/is_fallback\s*=\s*(true|false)/)?.[1] === "true";
          const storedHash = raw.match(/config_hash\s*=\s*"([^"]+)"/)?.[1];
          configOk = !storedHash || storedHash === computeConfigHash(pkgConfig);

          if (!configOk) {
            status = "Outdated";
            reason = "Config changed (resync needed)";
          } else if (isFallback) {
            status = "SyncedFallback";
            reason = "Synced (fallback: main/master)";
          }

          if (syncMode === "latest_docs") {
            const fetchedAtMatch = raw.match(/fetched_at\s*=\s*"([^"]+)"/)?.[1];
            if (fetchedAtMatch && !isLatestCacheFresh(fetchedAtMatch, config.settings.latest_ttl_hours)) {
              status = "Outdated";
              reason = "Cache TTL expired";
            }
          }
        } catch {
          status = "ReadError";
          reason = "Failed to read metadata";
        }
      }
    }

    statuses.push({
      name,
      lockVersion: targetVersion,
      status,
      reason,
      configOk,
      isFallback,
    });
  }

  if (format === "json") {
    console.log(JSON.stringify(buildStatusReport(statuses), null, 2));
  } else {
    printStatusTable(statuses, config.settings.output_dir, syncMode);
  }
}

function isLatestCacheFresh(fetchedAt: string, ttlHours: number): boolean {
  try {
    const date = new Date(fetchedAt);
    if (isNaN(date.getTime())) return false;
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    return diffMs < ttlHours * 60 * 60 * 1000;
  } catch {
    return false;
  }
}

function buildStatusReport(statuses: PackageStatus[]): StatusReport {
  return {
    summary: {
      total: statuses.length,
      synced: statuses.filter((s) => s.status === "Synced" || s.status === "SyncedFallback").length,
      problems: statuses.filter((s) => s.status !== "Synced" && s.status !== "SyncedFallback").length,
    },
    packages: statuses,
  };
}

function printStatusTable(statuses: PackageStatus[], outputDir: string, syncMode: string): void {
  const nameWidth = 28;
  const verWidth = 15;

  console.log(chalk.gray(`Sync mode: ${syncMode}`));
  console.log(`${"Package".padEnd(nameWidth)} ${"Target Version".padEnd(verWidth)} Docs Status`);
  console.log("─".repeat(nameWidth + verWidth + 30));

  for (const s of statuses) {
    let statusStr: string;
    switch (s.status) {
      case "Synced":
        statusStr = chalk.green("✅ Synced");
        break;
      case "SyncedFallback":
        statusStr = chalk.yellow("⚠️ Synced (fallback)");
        break;
      case "Outdated":
        statusStr = chalk.yellow("⚠️ Outdated");
        break;
      case "Missing":
        statusStr = chalk.red("❌ Missing");
        break;
      case "Incomplete":
        statusStr = chalk.red("❌ Incomplete");
        break;
      case "ReadError":
        statusStr = chalk.red("❌ Read Error");
        break;
      default:
        statusStr = s.status;
    }

    console.log(`${s.name.padEnd(nameWidth)} ${(s.lockVersion ?? "N/A").padEnd(verWidth)} ${statusStr}`);
    if (s.status !== "Synced") {
      console.log(chalk.gray(`  ↳ ${s.reason}`));
    }
  }

  const report = buildStatusReport(statuses);
  console.log(
    `\nTotal: ${report.summary.total} | Synced: ${report.summary.synced} | Problems: ${report.summary.problems}`
  );

  if (report.summary.problems > 0) {
    console.log(chalk.yellow("\n💡 Hint: run `ai-fdocs sync` to update docs."));
  }

  console.log();
  if (process.env.GITHUB_TOKEN || process.env.GH_TOKEN) {
    console.log(chalk.gray("🔑 GitHub token: active"));
  } else {
    console.log(chalk.gray("🔑 GitHub token: not set (rate limits apply)"));
  }
}
