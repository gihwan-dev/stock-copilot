import { spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { planShards } from "./diff-mutants-plan.mjs";

const COMMIT = /^[0-9a-f]{40}$/u;
const NON_NEGATIVE = /^\d+$/u;
const POSITIVE = /^[1-9]\d*$/u;
const REPOSITORY = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/u;

export const PLAN_JOB_NAME = "diff-mutants-plan";
export const PLAN_REQUIRED_STEPS = ["range", "diff mutants plan"];
export const SHARD_REQUIRED_STEPS = [
  "diff mutants shard",
  "diff mutants summary",
];

export const parseJobPages = (pages) => {
  if (!Array.isArray(pages) || pages.length === 0) {
    throw new Error("작업 목록 페이지가 비어 있다");
  }
  let expectedTotal;
  const combined = [];
  for (const page of pages) {
    if (
      typeof page !== "object" ||
      page === null ||
      !Number.isSafeInteger(page.total_count) ||
      page.total_count < 0 ||
      !Array.isArray(page.jobs)
    ) {
      throw new Error("작업 목록 응답 형식이 올바르지 않다");
    }
    if (expectedTotal === undefined) {
      expectedTotal = page.total_count;
    } else if (expectedTotal !== page.total_count) {
      throw new Error("작업 목록 페이지의 total_count가 일치하지 않는다");
    }
    combined.push(...page.jobs);
  }
  if (combined.length !== expectedTotal) {
    throw new Error(
      `작업 목록이 불완전하다: 기대 ${expectedTotal}, 수신 ${combined.length}`,
    );
  }
  return combined;
};

const parseNonNegative = (value, label) => {
  const text = typeof value === "number" ? String(value) : value;
  if (typeof text !== "string" || !NON_NEGATIVE.test(text)) {
    return { error: `${label} 값이 정수가 아니다: ${value}` };
  }
  return { value: Number.parseInt(text, 10) };
};

const parseShardArray = (value, label) => {
  if (Array.isArray(value)) {
    return { value };
  }
  if (typeof value !== "string" || value.length === 0) {
    return { error: `${label} 목록이 비어 있다` };
  }
  try {
    const parsed = JSON.parse(value);
    if (!Array.isArray(parsed)) {
      return { error: `${label} 목록이 배열이 아니다` };
    }
    return { value: parsed };
  } catch {
    return { error: `${label} 목록 JSON을 읽지 못했다` };
  }
};

export const expectedShardJobs = (linuxShards, macosShards) => [
  ...linuxShards.map((shard) => `diff-mutants-linux (${shard})`),
  ...macosShards.map((shard) => `diff-mutants-macos (${shard})`),
];

const verifyJob = (jobs, name, sha, attempt, requiredSteps) => {
  const matches = jobs.filter((job) => job?.name === name);
  if (matches.length === 0) {
    return [`필수 작업이 누락됐다: ${name}`];
  }
  if (matches.length > 1) {
    return [`필수 작업이 중복됐다: ${name} (${matches.length}건)`];
  }
  const [job] = matches;
  const problems = [];
  if (job.head_sha !== sha) {
    problems.push(`${name}: head_sha가 다르다`);
  }
  if (job.run_attempt !== attempt) {
    problems.push(`${name}: run_attempt가 다르다`);
  }
  if (job.status !== "completed" || job.conclusion !== "success") {
    problems.push(
      `${name}: 완료 성공이 아니다(status=${job.status}, conclusion=${job.conclusion})`,
    );
  }
  if (!Array.isArray(job.steps)) {
    problems.push(`${name}: 단계 목록이 없다`);
    return problems;
  }
  for (const stepName of requiredSteps) {
    const found = job.steps.filter((step) => step?.name === stepName);
    if (found.length !== 1) {
      problems.push(`${name}: 필수 단계 '${stepName}'가 ${found.length}건이다`);
    } else if (
      found[0].status !== "completed" ||
      found[0].conclusion !== "success"
    ) {
      problems.push(
        `${name}: 필수 단계 '${stepName}'가 성공하지 않았다(status=${found[0].status}, conclusion=${found[0].conclusion})`,
      );
    }
  }
  return problems;
};

const isAnyShardJobName = (name) =>
  typeof name === "string" &&
  (name.startsWith("diff-mutants-linux") ||
    name.startsWith("diff-mutants-macos"));

export const evaluateAggregate = (input, jobs) => {
  const problems = [];
  const sha = input?.sha ?? "";
  if (typeof sha !== "string" || !COMMIT.test(sha)) {
    problems.push("대상 40자리 커밋 SHA가 올바르지 않다");
  }
  const attemptRaw =
    typeof input?.runAttempt === "number"
      ? String(input.runAttempt)
      : (input?.runAttempt ?? "");
  const attempt = POSITIVE.test(attemptRaw)
    ? Number.parseInt(attemptRaw, 10)
    : undefined;
  if (attempt === undefined) {
    problems.push(`실행 회차가 양의 정수가 아니다: ${input?.runAttempt}`);
  }
  if (input?.planResult !== "success") {
    problems.push(
      `diff-mutants-plan이 성공하지 않았다: ${input?.planResult ?? "없음"}`,
    );
  }
  const total = parseNonNegative(input?.total, "MUTATION_TOTAL");
  const linuxCount = parseNonNegative(input?.linuxCount, "LINUX_COUNT");
  const macosCount = parseNonNegative(input?.macosCount, "MACOS_COUNT");
  for (const parsed of [total, linuxCount, macosCount]) {
    if (parsed.error !== undefined) {
      problems.push(parsed.error);
    }
  }
  const linuxShards = parseShardArray(input?.linuxShards, "LINUX_SHARDS");
  const macosShards = parseShardArray(input?.macosShards, "MACOS_SHARDS");
  for (const parsed of [linuxShards, macosShards]) {
    if (parsed.error !== undefined) {
      problems.push(parsed.error);
    }
  }
  let expectedNames = [];
  if (
    total.value !== undefined &&
    linuxCount.value !== undefined &&
    macosCount.value !== undefined &&
    linuxShards.value !== undefined &&
    macosShards.value !== undefined
  ) {
    if (linuxCount.value + macosCount.value !== total.value) {
      problems.push(
        `플랫폼별 변이 합(${linuxCount.value} + ${macosCount.value})이 전체(${total.value})와 다르다`,
      );
    }
    const expectedLinux = planShards(linuxCount.value);
    const expectedMacos = planShards(macosCount.value);
    if (JSON.stringify(linuxShards.value) !== JSON.stringify(expectedLinux)) {
      problems.push("LINUX_SHARDS가 계획 규칙과 일치하지 않는다");
    }
    if (JSON.stringify(macosShards.value) !== JSON.stringify(expectedMacos)) {
      problems.push("MACOS_SHARDS가 계획 규칙과 일치하지 않는다");
    }
    const expectedRunLinux = linuxCount.value > 0 ? "yes" : "no";
    const expectedRunMacos = macosCount.value > 0 ? "yes" : "no";
    if (input?.runLinux !== expectedRunLinux) {
      problems.push(
        `RUN_LINUX(${input?.runLinux})가 기대값(${expectedRunLinux})과 다르다`,
      );
    }
    if (input?.runMacos !== expectedRunMacos) {
      problems.push(
        `RUN_MACOS(${input?.runMacos})가 기대값(${expectedRunMacos})과 다르다`,
      );
    }
    const expectedLinuxResult = linuxCount.value > 0 ? "success" : "skipped";
    const expectedMacosResult = macosCount.value > 0 ? "success" : "skipped";
    if (input?.linuxResult !== expectedLinuxResult) {
      problems.push(
        `diff-mutants-linux 결과(${input?.linuxResult})가 기대값(${expectedLinuxResult})과 다르다`,
      );
    }
    if (input?.macosResult !== expectedMacosResult) {
      problems.push(
        `diff-mutants-macos 결과(${input?.macosResult})가 기대값(${expectedMacosResult})과 다르다`,
      );
    }
    expectedNames = expectedShardJobs(expectedLinux, expectedMacos);
  }
  if (!Array.isArray(jobs)) {
    problems.push("작업 목록이 배열이 아니다");
  } else if (attempt !== undefined && COMMIT.test(sha)) {
    problems.push(
      ...verifyJob(jobs, PLAN_JOB_NAME, sha, attempt, PLAN_REQUIRED_STEPS),
    );
    const expectedSet = new Set(expectedNames);
    for (const name of expectedNames) {
      problems.push(
        ...verifyJob(jobs, name, sha, attempt, SHARD_REQUIRED_STEPS),
      );
    }
    for (const job of jobs) {
      if (isAnyShardJobName(job?.name) && !expectedSet.has(job.name)) {
        problems.push(`계획에 없는 변이 조각 작업이 존재한다: ${job.name}`);
      }
    }
  }
  const ok = problems.length === 0;
  const rows = expectedNames.map((name) => {
    const matched = Array.isArray(jobs)
      ? jobs.find((job) => job?.name === name)
      : undefined;
    const status =
      matched === undefined
        ? "누락"
        : `${matched.status}/${matched.conclusion}`;
    return `| ${name} | ${status} |`;
  });
  const lines = [
    "## 변경분 변이 최종 판정",
    "",
    `- 판정: ${ok ? "통과" : "실패"}`,
    `- 전체 변이: ${total.value ?? "알 수 없음"}개 (Linux ${linuxCount.value ?? "?"}개 / macOS ${macosCount.value ?? "?"}개)`,
    `- 계획 조각: ${expectedNames.length}개`,
  ];
  if (rows.length > 0) {
    lines.push("", "| 조각 작업 | 결과 |", "|---|---|", ...rows);
  }
  if (problems.length > 0) {
    lines.push("", "### 실패 사유", "", ...problems.map((item) => `- ${item}`));
  }
  lines.push("");
  return { ok, problems, summary: lines.join("\n") };
};

export const fetchRunJobs = (repository, runId, attempt) => {
  if (!REPOSITORY.test(repository)) {
    throw new Error(`저장소 이름이 올바르지 않다: ${repository}`);
  }
  if (!POSITIVE.test(String(runId)) || !POSITIVE.test(String(attempt))) {
    throw new Error("실행 ID 또는 회차가 올바르지 않다");
  }
  const result = spawnSync(
    "gh",
    [
      "api",
      "--paginate",
      "--slurp",
      `repos/${repository}/actions/runs/${runId}/attempts/${attempt}/jobs?per_page=100`,
    ],
    { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  );
  if (result.status !== 0) {
    throw new Error(
      `GitHub Actions 작업 목록 조회 실패: ${result.stderr.trim()}`,
    );
  }
  return parseJobPages(JSON.parse(result.stdout));
};

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    const jobs = fetchRunJobs(
      process.env.REPOSITORY ?? "",
      process.env.RUN_ID ?? "",
      process.env.RUN_ATTEMPT ?? "",
    );
    const verdict = evaluateAggregate(
      {
        sha: process.env.SHA ?? "",
        runAttempt: process.env.RUN_ATTEMPT ?? "",
        planResult: process.env.PLAN_RESULT ?? "",
        linuxResult: process.env.LINUX_RESULT ?? "",
        macosResult: process.env.MACOS_RESULT ?? "",
        total: process.env.MUTATION_TOTAL ?? "",
        linuxCount: process.env.LINUX_COUNT ?? "",
        macosCount: process.env.MACOS_COUNT ?? "",
        linuxShards: process.env.LINUX_SHARDS ?? "",
        macosShards: process.env.MACOS_SHARDS ?? "",
        runLinux: process.env.RUN_LINUX ?? "",
        runMacos: process.env.RUN_MACOS ?? "",
      },
      jobs,
    );
    process.stdout.write(verdict.summary);
    if (
      typeof process.env.GITHUB_STEP_SUMMARY === "string" &&
      process.env.GITHUB_STEP_SUMMARY.length > 0
    ) {
      appendFileSync(process.env.GITHUB_STEP_SUMMARY, verdict.summary);
    }
    if (!verdict.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    const message = `diff-mutants-aggregate: ${error instanceof Error ? error.message : String(error)}\n`;
    process.stderr.write(message);
    if (
      typeof process.env.GITHUB_STEP_SUMMARY === "string" &&
      process.env.GITHUB_STEP_SUMMARY.length > 0
    ) {
      appendFileSync(
        process.env.GITHUB_STEP_SUMMARY,
        `## 변경분 변이 최종 판정\n\n- 판정: 실패\n- 사유: ${message}`,
      );
    }
    process.exitCode = 1;
  }
}
