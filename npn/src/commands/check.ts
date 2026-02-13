import { join } from "node:path";
import { existsSync, readFileSync } from "node:fs";
import chalk from "chalk";
import { loadConfig } from "../config.js";
import { resolveVersions } from "../resolver.js";
import { computeConfigHash } from "../config-hash.js";
import { isCachedV2 } from "../storage.js";
import { AiDocsError } from "../error.js";
import { NpmRegistryClient } from "../registry.js";

interface CheckIssue {
  name: string;
  kind: "missing" | "config_changed" | "not_in_lockfile";
}

export interface CheckReport {
  ok: boolean;
  issues: CheckIssue[];
}

export type CheckFormat = "text" | "json";

export async function buildCheckReport(projectRoot: string, modeOverride?: string): Promise<CheckReport> {
  const config = loadConfig(projectRoot);
  const syncMode = modeOverride || config.settings.sync_mode;
  const outputDir = join(projectRoot, config.settings.output_dir);

  const registry = new NpmRegistryClient();
  let targetVersions: Map<string, string>;

  if (syncMode === "latest_docs") {
    targetVersions = new Map();
    for (const name of Object.keys(config.packages)) {
      try {
        const ver = await registry.getLatestVersion(name);
        targetVersions.set(name, ver);
      } catch {
        // failed to resolve
      }
    }
  } else {
    targetVersions = resolveVersions(projectRoot);
  }

  const issues: CheckIssue[] = [];

  for (const [name, pkgConfig] of Object.entries(config.packages)) {
    const version = targetVersions.get(name);
    if (!version) {
      issues.push({ name, kind: syncMode === "latest_docs" ? "missing" : "not_in_lockfile" });
      continue;
    }

    const pkgDir = join(outputDir, `${name}@${version}`);
    const metaPath = join(pkgDir, ".aifd-meta.toml");

    if (!existsSync(pkgDir) || !existsSync(metaPath)) {
      issues.push({ name, kind: "missing" });
      continue;
    }

    if (!isCachedV2(outputDir, name, version, computeConfigHash(pkgConfig))) {
      issues.push({ name, kind: "config_changed" });
      continue;
    }

    if (syncMode === "latest_docs") {
      try {
        const raw = readFileSync(metaPath, "utf-8");
        const fetchedAtMatch = raw.match(/fetched_at\s*=\s*"([^"]+)"/)?.[1];
        if (fetchedAtMatch) {
          const date = new Date(fetchedAtMatch);
          const now = new Date();
          const diffMs = now.getTime() - date.getTime();
          if (diffMs > config.settings.latest_ttl_hours * 60 * 60 * 1000) {
            issues.push({ name, kind: "config_changed" }); // Treat expired TTL as needing update
          }
        }
      } catch {
        issues.push({ name, kind: "missing" });
      }
    }
  }

  return {
    ok: issues.length === 0,
    issues,
  };
}

function formatIssue(issue: CheckIssue): string {
  if (issue.kind === "missing") return `${issue.name}: docs missing`;
  if (issue.kind === "not_in_lockfile") return `${issue.name}: not in lockfile`;
  return `${issue.name}: outdated (config changed or TTL expired)`;
}

function renderTextReport(report: CheckReport): void {
  if (report.ok) {
    console.log(chalk.green("✅ All documentation is up-to-date."));
    return;
  }

  console.error(chalk.red("❌ Documentation is outdated:"));
  for (const issue of report.issues) console.error(chalk.red(`  - ${formatIssue(issue)}`));
  console.error(chalk.yellow("Run `ai-fdocs sync` to fix."));
}

export function renderJsonReport(report: CheckReport): string {
  return JSON.stringify(report, null, 2);
}

export async function cmdCheck(projectRoot: string, format: string = "text", modeOverride?: string): Promise<void> {
  if (format !== "text" && format !== "json") {
    throw new AiDocsError(`Unsupported --format value: ${format}`, "INVALID_FORMAT");
  }

  const report = await buildCheckReport(projectRoot, modeOverride);

  if (format === "json") {
    console.log(renderJsonReport(report));
  } else {
    renderTextReport(report);
  }

  process.exit(report.ok ? 0 : 1);
}
