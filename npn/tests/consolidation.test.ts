import { describe, it, expect, vi, beforeEach } from "vitest";
import { generateConsolidatedDoc, countTokens } from "../src/consolidation.js";
import { SummaryData } from "../src/summary.js";
import * as fs from "node:fs";
import * as path from "node:path";

vi.mock("node:fs");
vi.mock("yaml", () => ({
    default: {
        stringify: (obj: any) => Object.entries(obj).map(([k, v]) => `${k}: ${JSON.stringify(v)}`).join("\n"),
    },
}));
vi.mock("gpt-tokenizer/model/gpt-4", () => ({
    encode: (text: string) => ({ length: text.length / 4 }), // mock length
}));

describe("Consolidation", () => {
    const pkgDir = "/test/pkg";
    const data: SummaryData = {
        packageName: "test-pkg",
        version: "1.0.0",
        repo: "user/repo",
        gitRef: "main",
        isFallback: false,
        aiNotes: "Useful notes",
        files: [
            { flatName: "readme.md", originalPath: "README.md", sizeBytes: 100 },
            { flatName: "index.ts", originalPath: "index.ts", sizeBytes: 50 },
        ],
    };

    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("should generate a consolidated doc with YAML frontmatter", async () => {
        const readmeContent = "# Hello\nWorld";
        vi.spyOn(fs, "existsSync").mockReturnValue(true);
        vi.spyOn(fs, "readFileSync").mockReturnValue(readmeContent);
        const writeSpy = vi.spyOn(fs, "writeFileSync");

        await generateConsolidatedDoc(pkgDir, data, { includeChangelog: false, normalizeMarkdown: false });

        const call = writeSpy.mock.calls[0];
        const content = call[1] as string;

        expect(content).toContain("test-pkg");
        expect(content).toContain("1.0.0");
        expect(content).toContain("## README");
        expect(content).toContain(readmeContent);
        expect(content).toContain("tokens:");
    });

    it("should calculate tokens correctly", () => {
        const text = "This is a test document with some content.";
        const tokens = countTokens(text);
        expect(tokens).toBeGreaterThan(0);
    });
});
