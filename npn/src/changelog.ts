export function truncateChangelog(content: string, _version: string): string {
  const maxLines = 400;
  const lines = content.split("\n");
  if (lines.length <= maxLines) return content;
  return `${lines.slice(0, maxLines).join("\n")}\n\n[TRUNCATED by ai-fdocs]\n`;
}
