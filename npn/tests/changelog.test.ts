import { describe, it, expect } from "vitest";
import { truncateChangelog } from "../src/changelog.js";

describe("truncateChangelog", () => {
    it("keeps current version and one previous minor series", () => {
        const changelog = `# Changelog

## [0.13.1] - 2024-01-15
- Fix bug

## [0.13.0] - 2024-01-01
- New feature

## [0.12.0] - 2023-12-01
- Old feature

## [0.11.0] - 2023-11-01
- Ancient feature
`;
        const result = truncateChangelog(changelog, "0.13.1");
        expect(result).toContain("0.13.1");
        expect(result).toContain("0.13.0");
        expect(result).toContain("0.12.0");
        expect(result).not.toContain("0.11.0");
        expect(result).toContain("*[Earlier entries truncated by ai-fdocs]*");
    });

    it("handles v-prefix and different heading levels", () => {
        const changelog = `# My Lib

### v2.1.0
- minor

## v2.0.1
- patch

# v2.0.0
- major

## v1.9.0
- previous minor
`;
        // Current: 2.1.0 (minor: 2.1)
        // Previous minor: 2.0 (contains 2.0.1, 2.0.0)
        // Cut before 1.9.0
        const result = truncateChangelog(changelog, "2.1.0");
        expect(result).toContain("v2.1.0");
        expect(result).toContain("v2.0.1");
        expect(result).toContain("v2.0.0");
        expect(result).not.toContain("v1.9.0");
    });

    it("returns as-is if no version headings found", () => {
        const content = "Just some text without versions.";
        const result = truncateChangelog(content, "1.0.0");
        expect(result).toBe(content);
    });

    it("falls back to first 2 headings if current version not found", () => {
        const changelog = `## 1.2.0
## 1.1.0
## 1.0.0
## 0.9.0`;
        const result = truncateChangelog(changelog, "9.9.9");
        expect(result).toContain("1.2.0");
        expect(result).toContain("1.1.0");
        expect(result).not.toContain("1.0.0");
    });
});
