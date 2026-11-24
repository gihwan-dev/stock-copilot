import { appendFileSync, existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import {
  CATALOG_FILE,
  EXCLUSIONS_FILE,
  excludedCrateSet,
  platformOfSource,
  validCatalog,
} from "./diff-mutants-plan.mjs";
import { sliceFor } from "./mutation-plan.mjs";

export const RECEIPT_FILE = "target/mutants/diff-shard.json";
export const RESULTS_DIR = "target/mutants/mutants.out";
export const EVIDENCE_FILES = [
  "missed.txt",
  "timeout.txt",
  "caught.txt",
  "unviable.txt",
];

const COMMIT = /^[0-9a-f]{40}$/u;
const PLATFORMS = ["linux", "macos"];

const identityList = (entries) =>
  entries.map((entry) => [entry.name, entry.file]);

export const verifyDiffShard = (
  catalog,
  receipt,
  planned,
  outcomes,
  evidence,
  expected,
  exclusions,
) => {
  if (typeof expected?.base !== "string" || !COMMIT.test(expected.base)) {
    throw new Error("변이 기준 40자리 SHA가 올바르지 않다");
  }
  if (typeof expected?.head !== "string" || !COMMIT.test(expected.head)) {
    throw new Error("변이 대상 40자리 SHA가 올바르지 않다");
  }
  if (!PLATFORMS.includes(expected.platform)) {
    throw new Error(`변이 플랫폼이 올바르지 않다: ${expected.platform}`);
  }
  if (
    !Number.isSafeInteger(expected.expectedCount) ||
    expected.expectedCount <= 0
  ) {
    throw new Error("기대 변이 수가 양의 정수가 아니다");
  }
  if (
    typeof receipt !== "object" ||
    receipt === null ||
    receipt.base !== expected.base ||
    receipt.head !== expected.head ||
    receipt.platform !== expected.platform ||
    receipt.shard !== expected.shard
  ) {
    throw new Error("변이 조각 영수증이 계획과 일치하지 않는다");
  }
  if (!validCatalog(catalog) || catalog.length !== expected.expectedCount) {
    throw new Error("조각 실행의 변이 목록이 계획과 다르다");
  }
  const excluded = excludedCrateSet(exclusions);
  if (
    catalog.some(
      (entry) => platformOfSource(entry.file, excluded) !== expected.platform,
    )
  ) {
    throw new Error("조각 실행에 다른 플랫폼의 변이가 섞였다");
  }
  const slice = sliceFor(catalog, expected.shard);
  if (
    !validCatalog(planned) ||
    JSON.stringify(identityList(planned)) !==
      JSON.stringify(identityList(slice))
  ) {
    throw new Error("실제 변이 조각이 계획과 다르다");
  }
  if (EVIDENCE_FILES.some((name) => typeof evidence?.[name] !== "string")) {
    throw new Error("변이 결과 증거 파일이 빠져 있다");
  }
  if (slice.length === 0) {
    if (
      outcomes !== undefined ||
      EVIDENCE_FILES.some((name) => evidence[name].trim() !== "")
    ) {
      throw new Error("빈 변이 조각의 결과가 비어 있지 않다");
    }
    return "대상 없음";
  }
  if (
    outcomes?.total_mutants !== slice.length ||
    !Array.isArray(outcomes.outcomes) ||
    !outcomes.outcomes.some(
      (entry) => entry.scenario === "Baseline" && entry.summary === "Success",
    )
  ) {
    throw new Error("변이 조각이 완주하지 않았다");
  }
  if (evidence["timeout.txt"].trim() !== "") {
    throw new Error("변이 조각에 시간 초과가 남아 있다");
  }
  return "완주";
};

export const collectEvidence = (root) =>
  Object.fromEntries(
    EVIDENCE_FILES.map((name) => {
      const path = join(root, RESULTS_DIR, name);
      return [name, existsSync(path) ? readFileSync(path, "utf8") : undefined];
    }),
  );

export const formatShardSummary = ({
  platform,
  shard,
  outcome,
  state,
  reason,
  evidence,
}) => {
  const details = EVIDENCE_FILES.map((name) => {
    const content = evidence[name] ?? "결과 파일 없음";
    return `### ${name}\n\n\`\`\`text\n${content}\n\`\`\`\n`;
  }).join("\n");
  const lines = [
    "## 변경분 변이 조각 결과",
    "",
    `플랫폼: ${platform ?? "알 수 없음"}, 조각: ${shard ?? "알 수 없음"}`,
    "",
    `실행: ${outcome ?? "알 수 없음"}, 상태: ${state}`,
  ];
  if (reason !== undefined && reason.length > 0) {
    lines.push("", `사유: ${reason}`);
  }
  lines.push("", details);
  return lines.join("\n");
};

const parseExpectedCount = (raw) => {
  if (typeof raw !== "string" || !/^[1-9]\d*$/u.test(raw)) {
    throw new Error(`기대 변이 수가 올바르지 않다: ${raw}`);
  }
  return Number.parseInt(raw, 10);
};

const readJsonFile = (root, relative) =>
  JSON.parse(readFileSync(join(root, relative), "utf8"));

export const summarizeShard = (root, env) => {
  const evidence = collectEvidence(root);
  const outcome = env.MUTATION_OUTCOME;
  let state;
  let reason;
  try {
    const expectedCount = parseExpectedCount(env.MUTATION_EXPECTED_COUNT ?? "");
    const catalog = readJsonFile(root, CATALOG_FILE);
    const receipt = readJsonFile(root, RECEIPT_FILE);
    const exclusions = readJsonFile(root, EXCLUSIONS_FILE);
    const planned = readJsonFile(root, join(RESULTS_DIR, "mutants.json"));
    const outcomesPath = join(root, RESULTS_DIR, "outcomes.json");
    const outcomes = existsSync(outcomesPath)
      ? JSON.parse(readFileSync(outcomesPath, "utf8"))
      : undefined;
    state = verifyDiffShard(
      catalog,
      receipt,
      planned,
      outcomes,
      evidence,
      {
        base: env.MUTATION_BASE ?? "",
        head: env.SHA ?? "",
        platform: env.MUTATION_PLATFORM ?? "",
        shard: env.MUTATION_SHARD ?? "",
        expectedCount,
      },
      exclusions,
    );
  } catch (error) {
    state = "불완전";
    reason = error instanceof Error ? error.message : String(error);
  }
  const summary = formatShardSummary({
    platform: env.MUTATION_PLATFORM,
    shard: env.MUTATION_SHARD,
    outcome,
    state,
    reason,
    evidence,
  });
  const ok =
    outcome === "success" && (state === "완주" || state === "대상 없음");
  return { ok, state, reason, summary };
};

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const [subcommand] = process.argv.slice(2);
  if (subcommand !== "summary") {
    process.stderr.write(
      "사용법: node harness/ci/diff-mutants-shard.mjs summary\n",
    );
    process.exitCode = 2;
  } else {
    const result = summarizeShard(process.cwd(), process.env);
    process.stdout.write(result.summary);
    if (
      typeof process.env.GITHUB_STEP_SUMMARY === "string" &&
      process.env.GITHUB_STEP_SUMMARY.length > 0
    ) {
      appendFileSync(process.env.GITHUB_STEP_SUMMARY, result.summary);
    }
    if (!result.ok) {
      process.exitCode = 1;
    }
  }
}
