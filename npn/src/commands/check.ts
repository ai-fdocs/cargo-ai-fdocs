import { join } from "node:path";
import { existsSync } from "node:fs";
import chalk from "chalk";
import { loadConfig } from "../config.js";
import { resolveVersions } from "../resolver.js";
import { computeConfigHash } from "../config-hash.js";
import { isCachedV2 } from "../storage.js";

interface CheckIssue {
  name: string;
  kind: "missing" | "config_changed" | "not_in_lockfile";
}

export async function cmdCheck(projectRoot: string): Promise<void> {
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

  if (issues.length === 0) {
    console.log(chalk.green("✅ All documentation is up-to-date."));
    process.exit(0);
  }

  console.error(chalk.red("❌ Documentation is outdated:"));
  for (const issue of issues) console.error(chalk.red(`  - ${formatIssue(issue)}`));
  console.error(chalk.yellow("Run `ai-fdocs sync` to fix."));
  process.exit(1);
}

function formatIssue(issue: CheckIssue): string {
  if (issue.kind === "missing") return `${issue.name}: docs missing`;
  if (issue.kind === "not_in_lockfile") return `${issue.name}: not in lockfile`;
  return `${issue.name}: config changed (resync needed)`;
}
