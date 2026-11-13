import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const PROGRESS_FILE = "docs/progress.md";

export const REVIEWS_DIRECTORY = "docs/reviews";

export const TABLE_HEADER = ["작업", "상태", "대상 커밋", "리뷰 기록"];

export const DONE_STATES = ["완료", "일부 완료"];

export const ROUND_DONE_STATE = /^완료 \(\d+회차까지\)$/u;

export const OPEN_STATES = ["해결"];

export const WAIVER_DECIDERS = ["사용자"];

export const ROUND_CELL = /^\d+(?: \(.+\))?$/u;

export const EMPTY_CELLS = [""];

export const PLACEHOLDER_CELLS = ["", "-"];

export const WAIVER_REASON_MINIMUM = 20;

export const COMMIT_CELL = /^([0-9a-f]{7,40})(?:$|\s)/u;

export const SECTIONS = [
  {
    name: "커밋 표",
    heading: /^## 커밋/u,
    table: ["회차", "기준 SHA", "대상 SHA"],
    filled: [0],
    commits: [1, 2],
    round: 0,
  },
  {
    name: "리뷰 표",
    heading: /^## 리뷰(?! 생략)/u,
    table: ["번호", "등급"],
    filled: [0, 1],
    commits: [],
    waiver: /^## 리뷰 생략/u,
  },
  {
    name: "측정 표",
    heading: /^## 측정/u,
    table: ["항목", "값"],
    filled: [0, 1],
    commits: [],
  },
];

const TASK_ID = /[A-Z]+-\d+(?:\([^)]*\))?/gu;

const TASK_LIST = /^[A-Z]+-\d+(?:\([^)]*\))?(?:, [A-Z]+-\d+(?:\([^)]*\))?)*$/u;

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

const isHeader = (row, header) =>
  header.every((name, position) => row[position] === name);

export const progressRows = (text) => {
  const rows = [];
  let header = undefined;
  let afterHeader = false;
  text.split("\n").forEach((line, index) => {
    const row = cells(line);
    if (row === undefined) {
      header = undefined;
      return;
    }
    if (header === undefined) {
      header = row;
      afterHeader = true;
      return;
    }
    if (afterHeader && isSeparator(row)) {
      afterHeader = false;
      return;
    }
    afterHeader = false;
    rows.push({
      line: index + 1,
      progress: isHeader(header, TABLE_HEADER),
      tasks: row[0] ?? "",
      state: row[1] ?? "",
      target: row[2] ?? "",
      record: row[3] ?? "",
    });
  });
  return rows;
};

export const isDone = (state) =>
  DONE_STATES.includes(state) || ROUND_DONE_STATE.test(state);

export const isKnownState = (state) =>
  isDone(state) || OPEN_STATES.includes(state);

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

const tableRows = (lines, header) => {
  const index = lines.findIndex((line) => {
    const row = cells(line);
    return row !== undefined && isHeader(row, header);
  });
  if (index === -1) {
    return undefined;
  }
  const separator = cells(lines[index + 1] ?? "");
  if (separator === undefined || !isSeparator(separator)) {
    return undefined;
  }
  const rows = [];
  for (const line of lines.slice(index + 2)) {
    const row = cells(line);
    if (row === undefined) {
      break;
    }
    rows.push(row);
  }
  return rows;
};

const commitOf = (root, cell) => {
  const match = COMMIT_CELL.exec(cell);
  if (match === null) {
    return undefined;
  }
  const run = spawnSync(
    "git",
    ["-C", root, "rev-parse", "--verify", "--quiet", `${match[1]}^{commit}`],
    { encoding: "utf8" },
  );
  return run.status === 0 ? run.stdout.trim() : undefined;
};

const strictAncestor = (root, older, newer) =>
  older !== newer &&
  spawnSync("git", ["-C", root, "merge-base", "--is-ancestor", older, newer], {
    stdio: "ignore",
  }).status === 0;

const tableProblems = (root, section, rows) => {
  if (rows === undefined || rows.length === 0) {
    return [`${section.name}에 행이 없다`];
  }
  const meaningful = rows.some((row) =>
    section.filled.every(
      (position) => !PLACEHOLDER_CELLS.includes(row[position] ?? ""),
    ),
  );
  return [
    ...(meaningful
      ? []
      : [`${section.name}에 내용이 있는 행이 없다('-'·빈 칸뿐)`]),
    ...rowProblems(root, section, rows),
  ];
};

const rowProblems = (root, section, rows) =>
  rows.flatMap((row, index) => {
    const label = `${section.name} ${index + 1}번째 행`;
    const commits = section.commits.map((position) =>
      commitOf(root, row[position] ?? ""),
    );
    return [
      ...(section.filled.every(
        (position) => !EMPTY_CELLS.includes(row[position] ?? ""),
      )
        ? []
        : [`${label}에 빈 칸이 있다`]),
      ...(section.round === undefined ||
      ROUND_CELL.test(row[section.round] ?? "")
        ? []
        : [`${label}의 회차는 숫자로 시작한다: ${row[section.round] ?? ""}`]),
      ...section.commits
        .filter((_, slot) => commits[slot] === undefined)
        .map(
          (position) =>
            `${label}의 ${section.table[position]}가 저장소에 있는 커밋(7~40자리 16진수)이 아니다: ${row[position] ?? ""}`,
        ),
      ...(commits.length === 2 &&
      commits.every((commit) => commit !== undefined) &&
      !strictAncestor(root, commits[0], commits[1])
        ? [`${label}의 기준 SHA가 대상 SHA의 조상이 아니다`]
        : []),
    ];
  });

export const waiverProblems = (lines) => {
  if (lines === undefined) {
    return undefined;
  }
  const decided = lines.some((line) =>
    WAIVER_DECIDERS.some((who) => line.trim() === `- 결정: ${who}`),
  );
  const reason = lines
    .map((line) => /^- 사유: (.+)$/u.exec(line.trim()))
    .find((match) => match !== null);
  return [
    ...(decided
      ? []
      : [`리뷰 생략 절에 '- 결정: ${WAIVER_DECIDERS.join("|")}' 줄이 없다`]),
    ...(reason !== undefined &&
    [...reason[1].trim()].length >= WAIVER_REASON_MINIMUM
      ? []
      : [
          `리뷰 생략 절에 ${WAIVER_REASON_MINIMUM}자 이상의 '- 사유: …' 줄이 없다`,
        ]),
  ];
};

export const recordProblems = (root, text, tasks) => {
  const lines = text.split("\n");
  const title = lines.find((line) => line.startsWith("# ")) ?? "";
  const missingIds = (tasks.match(TASK_ID) ?? []).filter(
    (id) => !title.includes(id),
  );
  const sections = SECTIONS.flatMap((section) => {
    const body = sectionLines(lines, section.heading);
    const rows =
      body === undefined ? undefined : tableRows(body, section.table);
    if (rows === undefined && section.waiver !== undefined) {
      const waived = waiverProblems(sectionLines(lines, section.waiver));
      if (waived !== undefined) {
        return waived;
      }
    }
    if (rows === undefined) {
      return [
        `${section.name}가 없다 (${section.heading.source.slice(1).replace("(?! 생략)", "")} 절과 | ${section.table.join(" | ")} | 표${section.waiver === undefined ? "" : ", 또는 결정과 사유를 적은 ## 리뷰 생략 절"})`,
      ];
    }
    return tableProblems(root, section, rows);
  });
  return [
    ...(missingIds.length === 0
      ? []
      : [`제목에 작업 ID가 없다: ${missingIds.join(", ")}`]),
    ...sections,
  ];
};

const progressRowProblems = (root, row) => {
  const where = `${PROGRESS_FILE}:${row.line} ${row.tasks}`;
  if (!row.progress) {
    return TASK_LIST.test(row.tasks)
      ? [`${where}: 작업 행은 '| ${TABLE_HEADER.join(" | ")} |' 표에만 둔다`]
      : [];
  }
  if (!isKnownState(row.state)) {
    return [
      `${where}: 상태 '${row.state}'는 정해진 값(${[...DONE_STATES, "완료 (N회차까지)", ...OPEN_STATES].join(", ")})이 아니다`,
    ];
  }
  if (!isDone(row.state)) {
    return [];
  }
  if (commitOf(root, row.target) === undefined) {
    return [`${where}: 대상 커밋이 저장소에 있는 커밋이 아니다: ${row.target}`];
  }
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
    root,
    readFileSync(join(root, target), "utf8"),
    row.tasks,
  ).map((problem) => `${where}: ${target}: ${problem}`);
};

export const reviewProblems = (root) => {
  const progressPath = join(root, PROGRESS_FILE);
  if (!existsSync(progressPath)) {
    return [`${PROGRESS_FILE}가 없다`];
  }
  const rows = progressRows(readFileSync(progressPath, "utf8"));
  if (!rows.some((row) => row.progress)) {
    return [`${PROGRESS_FILE}에 진행 표가 없다`];
  }
  return rows.flatMap((row) => progressRowProblems(root, row));
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
