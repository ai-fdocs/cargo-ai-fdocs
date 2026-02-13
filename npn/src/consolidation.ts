import { writeFileSync, readFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { SummaryData } from "./summary.js";
import yaml from "yaml";
import { encode } from "gpt-tokenizer/model/gpt-4";
import { remark } from "remark";
import remarkGfm from "remark-gfm";
import remarkStringify from "remark-stringify";
import TurndownService from "turndown";
import jsdoc2md from "jsdoc-to-markdown";
import { readdirSync } from "node:fs";

export interface ConsolidationOptions {
    includeChangelog: boolean;
    normalizeMarkdown: boolean;
}

export function countTokens(text: string): number {
    try {
        return encode(text).length;
    } catch {
        return 0;
    }
}

export async function cleanMarkdown(text: string): Promise<string> {
    const file = await remark()
        .use(remarkGfm)
        .use(remarkStringify)
        .process(text);
    return String(file);
}

export function convertHtmlToMarkdown(html: string): string {
    const turndown = new TurndownService();
    return turndown.turndown(html);
}

export async function generateConsolidatedDoc(pkgDir: string, data: SummaryData, options: ConsolidationOptions): Promise<void> {
    const { includeChangelog, normalizeMarkdown } = options;

    let content = "";

    content += `# ${data.packageName} Full Documentation\n\n`;

    // 2. Metadata / Description
    if (data.aiNotes) {
        content += `## AI Guidance\n\n${data.aiNotes}\n\n`;
    }

    // 3. Main README (pinned as priority)
    const readmeFile = data.files.find(f => f.originalPath.toLowerCase() === "readme.md");
    if (readmeFile) {
        const readmePath = join(pkgDir, readmeFile.flatName);
        if (existsSync(readmePath)) {
            content += `## README\n\n`;
            let readmeContent = readFileSync(readmePath, "utf-8");
            content += `${readmeContent}\n\n`;
        }
    } else {
        // Fallback: JSDoc to Markdown
        try {
            const files = readdirSync(pkgDir);
            const sourceFiles = files.filter(f => f.endsWith(".ts") || f.endsWith(".js"));
            if (sourceFiles.length > 0) {
                const sourcePaths = sourceFiles.map(f => join(pkgDir, f));
                const jsdocContent = await jsdoc2md.render({ files: sourcePaths });
                if (jsdocContent) {
                    content += `## Documentation (Generated from JSDoc)\n\n`;
                    content += `${jsdocContent}\n\n`;
                }
            }
        } catch (e) {
            // Ignore JSDoc generation errors, it's a best-effort fallback
        }
    }

    // 4. Changelog
    if (includeChangelog) {
        const changelogFile = data.files.find(f =>
            f.originalPath.toLowerCase().includes("changelog.md") ||
            f.originalPath.toLowerCase().includes("changes.md")
        );
        if (changelogFile) {
            const changelogPath = join(pkgDir, changelogFile.flatName);
            if (existsSync(changelogPath)) {
                content += `## Changelog\n\n`;
                content += readFileSync(changelogPath, "utf-8") + "\n\n";
            }
        }
    }

    // 5. Other Docs
    const otherFiles = data.files.filter(f => {
        const lower = f.originalPath.toLowerCase();
        const isMd = lower.endsWith(".md");
        const isHtml = lower.endsWith(".html") || lower.endsWith(".htm");

        return (isMd || isHtml) &&
            lower !== "readme.md" &&
            !lower.includes("changelog.md") &&
            !lower.includes("changes.md") &&
            !lower.includes("_summary.md");
    });

    if (otherFiles.length > 0) {
        content += `## Additional Documentation\n\n`;
        for (const file of otherFiles) {
            const filePath = join(pkgDir, file.flatName);
            if (existsSync(filePath)) {
                content += `### File: ${file.originalPath}\n\n`;
                let fileContent = readFileSync(filePath, "utf-8");
                if (file.originalPath.toLowerCase().endsWith(".html") || file.originalPath.toLowerCase().endsWith(".htm")) {
                    fileContent = convertHtmlToMarkdown(fileContent);
                }
                content += `${fileContent}\n\n`;
            }
        }
    }

    // 6. Normalization/cleaning via remark if normalizeMarkdown is true
    if (normalizeMarkdown) {
        content = await cleanMarkdown(content);
    }

    // 7. Final Assembly with Frontmatter including tokens
    const tokens = countTokens(content);
    const frontmatter = {
        name: data.packageName,
        version: data.version,
        repository: data.repo ? `https://github.com/${data.repo}` : undefined,
        tokens,
        generated: new Date().toISOString(),
        ai_notes: data.aiNotes || undefined,
    };

    const finalContent = `---\n${yaml.stringify(frontmatter)}---\n\n${content}`;
    writeFileSync(join(pkgDir, "llms-full.md"), finalContent, "utf-8");
}
