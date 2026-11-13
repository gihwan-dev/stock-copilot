import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const PROGRESS_FILE = "docs/progress.md";

export const REVIEWS_DIRECTORY = "docs/reviews";

export const TABLE_HEADER = ["작업", "상태", "대상 커밋", "리뷰 기록"];

export const DONE_STATES = ["완료", "일부 완료"];

export const SECTIONS = [
  {
    name: "커밋 표",
    heading: /^## 커밋/u,
    table: ["회차", "기준 SHA", "대상 SHA"],
  },
  {
    name: "리뷰 표",
    heading: /^## 리뷰/u,
    table: ["번호", "등급"],
    waiver: /^## 리뷰 생략/u,
  },
  { name: "측정 표", heading: /^## 측정/u, table: ["항목", "값"] },
];

const TASK_ID = /[A-Z]+-\d+(?:\([^)]*\))?/gu;

const LINK = /^\[[^\]]+\]\(([^)\s]+)\)$/u;

const cells = (line) =>
  line.trim().startsWith("|") && line.trim().endsWith("|")
    ? line
        .trim()
        .slice(1, -1)
        .split("|")
        .map((cell) => cell.trim())
    : undefined;

const isSeparator = (row) => row.every((cell) => /^:?-+:?$/u.test(cell));

export const progressRows = (text) => {
  const lines = text.split("\n");
  const rows = [];
  let inTable = false;
  lines.forEach((line, index) => {
    const row = cells(line);
    if (row === undefined) {
      inTable = false;
      return;
    }
    if (TABLE_HEADER.every((name, position) => row[position] === name)) {
      inTable = true;
      return;
    }
    if (inTable && !isSeparator(row)) {
      rows.push({
        line: index + 1,
        tasks: row[0] ?? "",
        state: row[1] ?? "",
        record: row[3] ?? "",
      });
    }
  });
  return rows;
};

export const isDone = (state) =>
  DONE_STATES.some((done) => state === done || state.startsWith(`${done} `));

const sectionLines = (lines, heading) => {
  const start = lines.findIndex((line) => heading.test(line));
  if (start === -1) {
    return undefined;
  }
  const end = lines.findIndex(
    (line, index) => index > start && /^## /u.test(line),
  );
  return lines.slice(start + 1, end === -1 ? lines.length : end);
};

const hasTable = (lines, header) => {
  const index = lines.findIndex((line) => {
    const row = cells(line);
    return (
      row !== undefined &&
      header.every((name, position) => row[position] === name)
    );
  });
  if (index === -1) {
    return false;
  }
  const separator = cells(lines[index + 1] ?? "");
  const first = cells(lines[index + 2] ?? "");
  return (
    separator !== undefined &&
    isSeparator(separator) &&
    first !== undefined &&
    first.some((cell) => cell !== "")
  );
};

const hasText = (lines) =>
  lines !== undefined && lines.some((line) => line.trim() !== "");

export const recordProblems = (text, tasks) => {
  const lines = text.split("\n");
  const title = lines.find((line) => line.startsWith("# ")) ?? "";
  const missingIds = (tasks.match(TASK_ID) ?? []).filter(
    (id) => !title.includes(id),
  );
  const sections = SECTIONS.flatMap((section) => {
    const body = sectionLines(lines, section.heading);
    if (body !== undefined && hasTable(body, section.table)) {
      return [];
    }
    if (
      section.waiver !== undefined &&
      hasText(sectionLines(lines, section.waiver))
    ) {
      return [];
    }
    return [
      `${section.name}가 없다 (${section.heading.source.slice(1)} 절과 | ${section.table.join(" | ")} | 표${section.waiver === undefined ? "" : ", 또는 사유를 적은 ## 리뷰 생략 절"})`,
    ];
  });
  return [
    ...(missingIds.length === 0
      ? []
      : [`제목에 작업 ID가 없다: ${missingIds.join(", ")}`]),
    ...sections,
  ];
};

export const reviewProblems = (root) => {
  const progressPath = join(root, PROGRESS_FILE);
  if (!existsSync(progressPath)) {
    return [`${PROGRESS_FILE}가 없다`];
  }
  const rows = progressRows(readFileSync(progressPath, "utf8"));
  if (rows.length === 0) {
    return [`${PROGRESS_FILE}에 진행 표가 없다`];
  }
  return rows
    .filter((row) => isDone(row.state))
    .flatMap((row) => {
      const where = `${PROGRESS_FILE}:${row.line} ${row.tasks}`;
      const link = LINK.exec(row.record);
      if (link === null) {
        return [`${where}: 완료 행에 리뷰 기록 링크가 없다`];
      }
      const target = normalize(join(dirname(PROGRESS_FILE), link[1]));
      if (dirname(target) !== REVIEWS_DIRECTORY || !target.endsWith(".md")) {
        return [
          `${where}: 리뷰 기록은 ${REVIEWS_DIRECTORY}/<작업 ID>.md다: ${link[1]}`,
        ];
      }
      if (!existsSync(join(root, target))) {
        return [`${where}: 리뷰 기록이 없다: ${target}`];
      }
      return recordProblems(
        readFileSync(join(root, target), "utf8"),
        row.tasks,
      ).map((problem) => `${where}: ${target}: ${problem}`);
    });
};

const repoRoot = resolve(
  fileURLToPath(import.meta.url),
  "..",
  "..",
  "..",
  "..",
);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = resolve(process.argv[2] ?? repoRoot);
  const problems = reviewProblems(root);
  problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
  process.stdout.write(`review-records: ${problems.length} failure(s)\n`);
  process.exitCode = problems.length === 0 ? 0 : 1;
}
