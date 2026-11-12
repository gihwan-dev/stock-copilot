import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HOOK_CASE_FILES = [
  "harness/claude-hooks/bash-cases.json",
  "harness/claude-hooks/edit-cases.json",
  "harness/claude-hooks/agent-cases.json",
];

const ESLINT_CASES = "harness/eslint-violations/cases.json";

const ADR_DIRECTORY = "docs/adr";

const GAP_HEADING = /^(#{2,4}) .*(알려진 빈틈|남는 한계|남는 빈틈)/u;

const readJson = (root, path) =>
  JSON.parse(readFileSync(join(root, path), "utf8"));

const eslintGaps = (root) =>
  Object.entries(readJson(root, ESLINT_CASES).cases)
    .filter(([, spec]) => spec.expect === "KNOWN_GAP")
    .map(([file, spec]) => ({
      source: "eslint",
      location: `harness/eslint-violations/${file}`,
      text: `${spec.row}: 보고돼야 할 규칙이 없다. 지금 보고: [${(spec.reports ?? []).join(", ")}]`,
    }));

const hookGaps = (root) =>
  HOOK_CASE_FILES.flatMap((path) =>
    readJson(root, path)
      .filter((item) => item.gap !== undefined)
      .map((item) => ({
        source: "hook",
        location: `${path} ${item.role ?? "main"}`,
        text: `${item.command ?? item.path ?? item.prompt ?? ""} → ${item.decision}: ${item.gap}`,
      })),
  );

const sectionItems = (lines, start) => {
  const level = GAP_HEADING.exec(lines[start])[1].length;
  const items = [];
  let tableRow = 0;
  for (let index = start + 1; index < lines.length; index += 1) {
    const line = lines[index];
    const heading = /^(#+) /u.exec(line);
    if (heading !== null && heading[1].length <= level) {
      break;
    }
    if (line.startsWith("- ")) {
      items.push({ line: index + 1, text: line.slice(2) });
    } else if (line.startsWith("|")) {
      tableRow += 1;
      if (tableRow > 2) {
        items.push({
          line: index + 1,
          text: line
            .split("|")
            .map((part) => part.trim())
            .filter((part) => part !== "")
            .join(" — "),
        });
      }
    } else {
      tableRow = 0;
    }
  }
  return items;
};

const adrGaps = (root) =>
  readdirSync(join(root, ADR_DIRECTORY))
    .filter((name) => name.endsWith(".md"))
    .sort()
    .flatMap((name) => {
      const path = `${ADR_DIRECTORY}/${name}`;
      const lines = readFileSync(join(root, path), "utf8").split("\n");
      return lines
        .map((line, index) => (GAP_HEADING.test(line) ? index : -1))
        .filter((index) => index >= 0)
        .flatMap((index) =>
          sectionItems(lines, index).map((item) => ({
            source: "adr",
            location: `${path}:${item.line}`,
            text: item.text,
          })),
        );
    });

export const knownGaps = (root) => [
  ...eslintGaps(root),
  ...hookGaps(root),
  ...adrGaps(root),
];

const cell = (text) => text.replaceAll("|", "\\|").replaceAll("\n", " ");

export const gapTable = (gaps) =>
  [
    "| 출처 | 위치 | 내용 |",
    "|---|---|---|",
    ...gaps.map(
      (gap) => `| ${gap.source} | ${cell(gap.location)} | ${cell(gap.text)} |`,
    ),
    `known-gaps: ${gaps.length}건 (${["eslint", "hook", "adr"]
      .map(
        (source) =>
          `${source} ${gaps.filter((gap) => gap.source === source).length}건`,
      )
      .join(", ")})`,
  ].join("\n");

const repoRoot = resolve(fileURLToPath(import.meta.url), "..", "..", "..");

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.stdout.write(`${gapTable(knownGaps(repoRoot))}\n`);
}
