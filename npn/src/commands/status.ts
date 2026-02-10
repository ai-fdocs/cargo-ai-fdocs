import { join } from "node:path";
import { existsSync, readFileSync } from "node:fs";
import chalk from "chalk";
import { loadConfig } from "../config.js";
import { resolveVersions } from "../resolver.js";
import { computeConfigHash } from "../config-hash.js";

export async function cmdStatus(projectRoot: string): Promise<void> {
  const config = loadConfig(projectRoot);
  const lockVersions = resolveVersions(projectRoot);
  const outputDir = join(projectRoot, config.settings.output_dir);

  const nameWidth = 28;
  const verWidth = 15;

  console.log(`${"Package".padEnd(nameWidth)} ${"Lock Version".padEnd(verWidth)} Docs Status`);
  console.log("─".repeat(nameWidth + verWidth + 30));

  for (const [name, pkgConfig] of Object.entries(config.packages)) {
    const lockVersion = lockVersions.get(name);
    const verStr = lockVersion ?? "N/A";

    let status: string;

    if (!lockVersion) {
      status = chalk.red("❌ Not in lockfile");
    } else {
      const pkgDir = join(outputDir, `${name}@${lockVersion}`);
      const metaPath = join(pkgDir, ".aifd-meta.toml");

      if (!existsSync(pkgDir)) {
        status = chalk.red("❌ Missing");
      } else if (!existsSync(metaPath)) {
        status = chalk.yellow("⚠️ Incomplete (no meta)");
      } else {
        try {
          const raw = readFileSync(metaPath, "utf-8");
          const fallback = raw.match(/is_fallback\s*=\s*(true|false)/)?.[1] === "true";
          const storedHash = raw.match(/config_hash\s*=\s*"([^"]+)"/)?.[1];
          const configOk = !storedHash || storedHash === computeConfigHash(pkgConfig);

          if (!configOk) status = chalk.yellow("⚠️ Config changed (resync needed)");
          else if (fallback) status = chalk.yellow("⚠️ Synced (fallback: main/master)");
          else status = chalk.green("✅ Synced");
        } catch {
          status = chalk.yellow("⚠️ Read error");
        }
      }
    }

    console.log(`${name.padEnd(nameWidth)} ${verStr.padEnd(verWidth)} ${status}`);
  }

  console.log();
  const gitattr = join(projectRoot, ".gitattributes");
  if (!existsSync(gitattr)) {
    console.log(chalk.gray("💡 Tip: Add to .gitattributes:"));
    console.log(chalk.gray(`   ${config.settings.output_dir}/** linguist-generated=true`));
  }

  if (process.env.GITHUB_TOKEN || process.env.GH_TOKEN) {
    console.log(chalk.gray("🔑 GitHub token: active (5000 req/hr)"));
  } else {
    console.log(chalk.gray("🔑 GitHub token: not set (60 req/hr)"));
  }
}
