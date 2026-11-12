import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { fixedFindings } from "../../.claude/hooks/lib/paths.mjs";
import { readRules } from "../../.claude/hooks/lib/project.mjs";

export const SKIPPED = [
  ".git",
  "node_modules",
  "target",
  "apps/desktop/dist",
  "apps/desktop/src-tauri/gen",
  ".claude/worktrees",
];

const skipped = (relative) =>
  SKIPPED.some(
    (prefix) =>
      relative === prefix ||
      relative.endsWith(`/${prefix}`) ||
      relative.startsWith(`${prefix}/`),
  );

const filesUnder = (root, relative = "") =>
  readdirSync(join(root, relative), { withFileTypes: true }).flatMap(
    (entry) => {
      const path = relative === "" ? entry.name : `${relative}/${entry.name}`;
      if (skipped(path)) {
        return [];
      }
      return entry.isDirectory() ? filesUnder(root, path) : [path];
    },
  );

export const misplacedConfigs = (root, rules) =>
  filesUnder(root)
    .filter((path) =>
      fixedFindings(path, rules).some(
        (item) => item.rule === "misplaced-config",
      ),
    )
    .sort();

const repoRoot = resolve(fileURLToPath(import.meta.url), "..", "..", "..");

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = resolve(process.argv[2] ?? repoRoot);
  const found = misplacedConfigs(root, readRules(repoRoot));
  found.forEach((path) =>
    process.stdout.write(
      `FAIL ${path}: 설정 파일 이름은 정해진 위치에만 둔다 (harness/claude-hooks/rules.json paths.configFiles, ADR 0017)\n`,
    ),
  );
  process.stdout.write(`config-files: ${found.length} failure(s)\n`);
  process.exitCode = found.length === 0 ? 0 : 1;
}
