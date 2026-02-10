#!/usr/bin/env node

import { Command } from "commander";
import chalk from "chalk";
import { AiDocsError } from "./error.js";

const program = new Command();

program.name("ai-fdocs").description("Sync documentation from npm dependencies for AI context").version("0.2.0");

program
  .command("init")
  .description("Generate ai-fdocs.toml from lockfile via npm registry")
  .option("--overwrite", "Overwrite existing config", false)
  .action(async (options) => {
    try {
      const { cmdInit } = await import("./commands/init.js");
      await cmdInit(process.cwd(), options.overwrite);
    } catch (e) {
      handleError(e);
    }
  });

program
  .command("sync")
  .description("Sync documentation based on lockfile")
  .option("-f, --force", "Force re-download ignoring cache", false)
  .action(async (options) => {
    try {
      const { cmdSync } = await import("./commands/sync.js");
      await cmdSync(process.cwd(), options.force);
    } catch (e) {
      handleError(e);
    }
  });

program
  .command("status")
  .description("Show current documentation status")
  .action(async () => {
    try {
      const { cmdStatus } = await import("./commands/status.js");
      await cmdStatus(process.cwd());
    } catch (e) {
      handleError(e);
    }
  });

program
  .command("check")
  .description("Check if docs are up-to-date (CI mode)")
  .action(async () => {
    try {
      const { cmdCheck } = await import("./commands/check.js");
      await cmdCheck(process.cwd());
    } catch (e) {
      handleError(e);
    }
  });

function handleError(e: unknown): void {
  if (e instanceof AiDocsError) {
    if (e.code === "CONFIG_NOT_FOUND") {
      console.error(chalk.yellow("ai-fdocs.toml not found."));
      console.error(chalk.gray("Run `ai-fdocs init` to generate one, or create manually."));
    } else {
      console.error(chalk.red(`Error [${e.code}]: ${e.message}`));
    }
  } else {
    console.error(chalk.red("Unexpected error:"), e);
  }
  process.exit(1);
}

program.parse();
