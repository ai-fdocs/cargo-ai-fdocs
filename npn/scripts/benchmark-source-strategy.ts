import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";
import { spawnSync } from "node:child_process";
import { parse } from "smol-toml";

type DocsSource = "github" | "npm_tarball";

interface BenchmarkPackage {
  name: string;
  version: string;
  repo: string;
}

interface SyncReport {
  source: DocsSource;
  totals: {
    synced: number;
    cached: number;
    skipped: number;
    errors: number;
  };
  sourceStats: Record<DocsSource, { synced: number; errors: number; skipped: number; cached: number }>;
  errorCodes: Record<string, number>;
  issues: string[];
}

interface ModeResult {
  mode: DocsSource;
  durationMs: number;
  report: SyncReport;
  usefulFiles: number;
}

const BENCHMARK_PACKAGES: BenchmarkPackage[] = [
  { name: "react", version: "18.3.1", repo: "facebook/react" },
  { name: "vue", version: "3.4.27", repo: "vuejs/core" },
  { name: "lodash", version: "4.17.21", repo: "lodash/lodash" },
  { name: "axios", version: "1.7.2", repo: "axios/axios" },
  { name: "express", version: "4.19.2", repo: "expressjs/express" },
  { name: "typescript", version: "5.6.3", repo: "microsoft/TypeScript" },
  { name: "vite", version: "5.4.10", repo: "vitejs/vite" },
  { name: "next", version: "14.2.15", repo: "vercel/next.js" },
  { name: "chalk", version: "5.3.0", repo: "chalk/chalk" },
  { name: "commander", version: "12.1.0", repo: "tj/commander.js" },
  { name: "zod", version: "3.23.8", repo: "colinhacks/zod" },
  { name: "rxjs", version: "7.8.1", repo: "ReactiveX/rxjs" },
  { name: "dayjs", version: "1.11.13", repo: "iamkun/dayjs" },
  { name: "date-fns", version: "3.6.0", repo: "date-fns/date-fns" },
  { name: "prettier", version: "3.3.3", repo: "prettier/prettier" },
  { name: "eslint", version: "9.12.0", repo: "eslint/eslint" },
  { name: "jest", version: "29.7.0", repo: "jestjs/jest" },
  { name: "vitest", version: "1.6.0", repo: "vitest-dev/vitest" },
  { name: "pinia", version: "2.1.7", repo: "vuejs/pinia" },
  { name: "nuxt", version: "3.13.2", repo: "nuxt/nuxt" },
  { name: "svelte", version: "4.2.19", repo: "sveltejs/svelte" },
  { name: "tailwindcss", version: "3.4.13", repo: "tailwindlabs/tailwindcss" },
];

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, "..");

function createPackageLock(packages: BenchmarkPackage[]): string {
  const packageEntries: Record<string, { version: string }> = {};
  const dependencyEntries: Record<string, { version: string }> = {};

  for (const pkg of packages) {
    packageEntries[`node_modules/${pkg.name}`] = { version: pkg.version };
    dependencyEntries[pkg.name] = { version: pkg.version };
  }

  return JSON.stringify(
    {
      name: "npn-source-strategy-benchmark",
      lockfileVersion: 3,
      requires: true,
      packages: {
        "": {
          name: "npn-source-strategy-benchmark",
          version: "0.0.0",
          dependencies: Object.fromEntries(packages.map((pkg) => [pkg.name, pkg.version])),
        },
        ...packageEntries,
      },
      dependencies: dependencyEntries,
    },
    null,
    2
  );
}

function createConfig(packages: BenchmarkPackage[], docsSource: DocsSource): string {
  const pkgLines = packages
    .map((pkg) => `[packages."${pkg.name}"]\nrepo = "${pkg.repo}"\n`)
    .join("\n");

  return `[settings]\noutput_dir = "docs/ai/vendor-docs/node"\nprune = true\nmax_file_size_kb = 512\nsync_concurrency = 6\ndocs_source = "${docsSource}"\n\n${pkgLines}`;
}

function countUsefulFiles(outputDir: string): number {
  let total = 0;
  for (const entry of readdirSync(outputDir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const metaPath = join(outputDir, entry.name, ".aifd-meta.toml");
    try {
      const metaRaw = readFileSync(metaPath, "utf-8");
      const parsed = parse(metaRaw) as { files?: unknown };
      if (Array.isArray(parsed.files)) {
        total += parsed.files.length;
      }
    } catch {
      // Missing metadata means no useful files for this package.
    }
  }

  return total;
}

function parseJsonFromStdout(stdout: string): SyncReport {
  const jsonStart = stdout.indexOf("{");
  if (jsonStart < 0) {
    throw new Error(`Expected JSON sync report in stdout, got:\n${stdout}`);
  }
  return JSON.parse(stdout.slice(jsonStart)) as SyncReport;
}

function runMode(rootDir: string, mode: DocsSource): ModeResult {
  const runDir = join(rootDir, mode);
  rmSync(runDir, { recursive: true, force: true });
  mkdirSync(runDir, { recursive: true });

  writeFileSync(join(runDir, "package-lock.json"), createPackageLock(BENCHMARK_PACKAGES));
  writeFileSync(join(runDir, "ai-fdocs.toml"), createConfig(BENCHMARK_PACKAGES, mode));

  const startedAt = process.hrtime.bigint();
  const syncResult = spawnSync("node", [join(projectRoot, "dist/cli.js"), "sync", "--force", "--report-format", "json"], {
    cwd: runDir,
    encoding: "utf-8",
    stdio: "pipe",
    env: process.env,
  });
  const durationMs = Number((process.hrtime.bigint() - startedAt) / BigInt(1_000_000));

  if (syncResult.status !== 0) {
    throw new Error(
      `sync failed for mode=${mode} (exit=${syncResult.status})\nstdout:\n${syncResult.stdout}\nstderr:\n${syncResult.stderr}`
    );
  }

  const report = parseJsonFromStdout(syncResult.stdout);
  const usefulFiles = countUsefulFiles(join(runDir, "docs/ai/vendor-docs/node"));

  return { mode, durationMs, report, usefulFiles };
}

function formatPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

function toErrorTable(results: ModeResult[]): string {
  const codes = new Set<string>();
  for (const result of results) {
    Object.keys(result.report.errorCodes).forEach((code) => codes.add(code));
  }

  if (codes.size === 0) {
    return "Ошибки по классам отсутствуют.";
  }

  const sortedCodes = [...codes].sort();
  const header = "| Error class | github | npm_tarball |\n|---|---:|---:|";
  const rows = sortedCodes.map((code) => {
    const gh = results.find((r) => r.mode === "github")?.report.errorCodes[code] ?? 0;
    const npm = results.find((r) => r.mode === "npm_tarball")?.report.errorCodes[code] ?? 0;
    return `| ${code} | ${gh} | ${npm} |`;
  });

  return [header, ...rows].join("\n");
}

function buildMarkdownReport(results: ModeResult[], reportPath: string): string {
  const generatedAt = new Date().toISOString();
  const total = BENCHMARK_PACKAGES.length;

  const resultRows = results
    .map((result) => {
      const successRate = (result.report.totals.synced + result.report.totals.cached) / total;
      return `| ${result.mode} | ${result.report.totals.synced} | ${result.report.totals.cached} | ${result.report.totals.skipped} | ${result.report.totals.errors} | ${formatPercent(successRate)} | ${result.durationMs} | ${result.usefulFiles} |`;
    })
    .join("\n");

  const packageRows = BENCHMARK_PACKAGES.map(
    (pkg, index) => `| ${index + 1} | ${pkg.name} | ${pkg.version} | ${pkg.repo} |`
  ).join("\n");

  const command = "npm run benchmark:source-strategy";

  return `# Source strategy benchmark (${new Date().toISOString().slice(0, 7)})

Дата генерации: ${generatedAt}.

## Как воспроизвести

1. Перейти в каталог \`npn/\`.
2. Выполнить \`${command}\`.
3. Скрипт создаёт временные benchmark-проекты, запускает \`sync\` в режимах \`docs_source=github\` и \`docs_source=npm_tarball\`, затем сохраняет этот отчёт в \`${reportPath}\`.

## Корпус (реальные npm-пакеты, ${total} шт.)

| # | package | version | repo |
|---:|---|---|---|
${packageRows}

## Сводные метрики

| mode | synced | cached | skipped | errors | success rate | duration (ms) | useful files |
|---|---:|---:|---:|---:|---:|---:|---:|
${resultRows}

## Ошибки по классам

${toErrorTable(results)}

## Примечания

- \`success rate\` = (synced + cached) / ${total}.
- \`useful files\` = сумма \`files\` из \`.aifd-meta.toml\` по всем синхронизированным пакетам.
`;
}

function main(): void {
  const build = spawnSync("npm", ["run", "build"], {
    cwd: projectRoot,
    encoding: "utf-8",
    stdio: "pipe",
  });

  if (build.status !== 0) {
    throw new Error(`Build failed\nstdout:\n${build.stdout}\nstderr:\n${build.stderr}`);
  }

  const tempRoot = mkdtempSync(join(tmpdir(), "npn-source-bench-"));

  try {
    const github = runMode(tempRoot, "github");
    const npmTarball = runMode(tempRoot, "npm_tarball");

    const reportName = `source-strategy-${new Date().toISOString().slice(0, 7)}.md`;
    const reportPath = join(projectRoot, "docs", "benchmarks", reportName);
    const markdown = buildMarkdownReport([github, npmTarball], `npn/docs/benchmarks/${reportName}`);
    writeFileSync(reportPath, markdown);

    console.log(`Benchmark report saved: ${reportPath}`);
  } finally {
    rmSync(tempRoot, { recursive: true, force: true });
  }
}

main();
