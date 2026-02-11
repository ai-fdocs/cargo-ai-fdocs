import { join } from "node:path";
import { existsSync } from "node:fs";
import chalk from "chalk";
import { loadConfig } from "../config.js";
import { resolveVersions } from "../resolver.js";
import { computeConfigHash } from "../config-hash.js";
import { isCachedV2 } from "../storage.js";
import { AiDocsError } from "../error.js";

interface CheckIssue {
  name: string;
  kind: "missing" | "config_changed" | "not_in_lockfile";
}

export interface CheckReport {
  ok: boolean;
  issues: CheckIssue[];
}

export type CheckFormat = "text" | "json";

export function buildCheckReport(projectRoot: string): CheckReport {
  const config = loadConfig(projectRoot);
  const lockVersions = resolveVersions(projectRoot);
  const outputDir = join(projectRoot, config.settings.output_dir);

  const issues: CheckIssue[] = [];

  for (const [name, pkgConfig] of Object.entries(config.packages)) {
    const version = lockVersions.get(name);
    if (!version) {
      issues.push({ name, kind: "not_in_lockfile" });
      continue;
    }

    const pkgDir = join(outputDir, `${name}@${version}`);
    if (!existsSync(pkgDir)) {
      issues.push({ name, kind: "missing" });
      continue;
    }

    if (!isCachedV2(outputDir, name, version, computeConfigHash(pkgConfig))) {
      issues.push({ name, kind: "config_changed" });
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
  return `${issue.name}: config changed (resync needed)`;
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

export async function cmdCheck(projectRoot: string, format: string = "text"): Promise<void> {
  if (format !== "text" && format !== "json") {
    throw new AiDocsError(`Unsupported --format value: ${format}`, "INVALID_FORMAT");
  }

  const report = buildCheckReport(projectRoot);

  if (format === "json") {
    console.log(renderJsonReport(report));
  } else {
    renderTextReport(report);
  }

  process.exit(report.ok ? 0 : 1);
}
