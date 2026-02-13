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
  .description("Sync documentation for dependencies")
  .option("--force", "Force re-download of all documentation")
  .option("--mode <mode>", "Sync mode: lockfile | latest_docs | hybrid", "lockfile")
  .option("--report-format <format>", "Output format for the sync report: text | json", "text")
  .action(async (options) => {
    try {
      const { cmdSync } = await import("./commands/sync.js");
      await cmdSync(process.cwd(), options);
    } catch (e) {
      handleError(e);
    }
  });

program
  .command("status")
  .description("Show current documentation status")
  .option("--format <format>", "Output format: text | json", "text")
  .option("--mode <mode>", "Sync mode override: lockfile | latest_docs | hybrid")
  .action(async (options) => {
    try {
      const { cmdStatus } = await import("./commands/status.js");
      await cmdStatus(process.cwd(), options.format, options.mode);
    } catch (e) {
      handleError(e);
    }
  });

program
  .command("check")
  .description("Check if documentation is up to date (exit code 1 if not)")
  .option("--format <format>", "Output format: text | json", "text")
  .option("--mode <mode>", "Sync mode override: lockfile | latest_docs | hybrid")
  .action(async (options) => {
    try {
      const { cmdCheck } = await import("./commands/check.js");
      await cmdCheck(process.cwd(), options.format, options.mode);
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
