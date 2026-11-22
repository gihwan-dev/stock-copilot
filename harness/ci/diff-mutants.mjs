import { spawnSync } from "node:child_process";
import { appendFileSync, readFileSync } from "node:fs";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";

const initialParts = 4;
const maximumParts = 128;
const targetPerPart = 7;
const shardBudget = 22 * 60_000;
const jobBudget = 115 * 60_000;
const output = "target/mutants";
const validCatalog = (catalog) =>
  Array.isArray(catalog) &&
  catalog.every(
    (item) =>
      typeof item?.name === "string" &&
      item.name.length > 0 &&
      typeof item.file === "string" &&
      item.file.length > 0,
  );

export const verifyShard = (
  catalog,
  previous,
  planned,
  outcomes,
  index,
  parts = initialParts,
) => {
  if (
    !validCatalog(catalog) ||
    ![4, 8, 16, 32, 64, 128].includes(parts) ||
    !Number.isInteger(index) ||
    index < 0 ||
    index >= parts
  )
    throw new Error("변경분 변이 목록 또는 조각이 올바르지 않다");
  if (
    previous !== undefined &&
    JSON.stringify(catalog) !== JSON.stringify(previous)
  )
    throw new Error("조각별 변이 목록이 다르다");
  if (catalog.length === 0) return;
  const chunk = Math.ceil(catalog.length / parts);
  const expected = catalog.slice(index * chunk, (index + 1) * chunk);
  if (
    !validCatalog(planned) ||
    JSON.stringify(planned) !== JSON.stringify(expected)
  )
    throw new Error("실제 변이 조각이 계획과 다르다");
  if (expected.length === 0) return;
  if (
    outcomes?.total_mutants !== expected.length ||
    !Array.isArray(outcomes.outcomes) ||
    !outcomes.outcomes.some(
      (entry) => entry.scenario === "Baseline" && entry.summary === "Success",
    )
  )
    throw new Error("변이 조각이 완주하지 않았다");
};

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const writeStatus = (status) => {
  if (
    typeof process.env.GITHUB_OUTPUT !== "string" ||
    process.env.GITHUB_OUTPUT.length === 0
  )
    throw new Error("GitHub 단계 출력이 없다");
  appendFileSync(process.env.GITHUB_OUTPUT, `status=${status}\n`);
};

export const jobDeadline = (started, now) => {
  if (typeof started !== "string" || !/^\d{10}$/u.test(started))
    throw new Error("작업 시작 시각이 없다");
  const start = Number(started) * 1000;
  if (!Number.isSafeInteger(now) || now < start || now > start + 120 * 60_000)
    throw new Error("작업 시작 시각이 올바르지 않다");
  return start + jobBudget;
};

export const runShards = (base, run, read, write, plan, budget) => {
  if (typeof base !== "string" || !/^[0-9a-f]{40}$/u.test(base))
    throw new Error("변이 기준 SHA가 없다");
  if (plan(base) !== 0) throw new Error("변경분 변이 계획 실패");
  const selected = read(`${output}/catalog.json`);
  if (!validCatalog(selected))
    throw new Error("변경분 변이 목록이 올바르지 않다");
  let parts = initialParts;
  while (
    Math.ceil(selected.length / parts) > targetPerPart &&
    parts < maximumParts
  )
    parts *= 2;
  if (Math.ceil(selected.length / parts) > targetPerPart)
    throw new Error("변경분이 너무 커서 작업을 나누어야 한다");
  while (true) {
    let retry = false;
    for (let index = 0; index < parts; index += 1) {
      const chunk = Math.ceil(selected.length / parts);
      if (selected.length > 0 && index * chunk >= selected.length) continue;
      if (budget !== undefined) {
        const remaining = budget.deadline - budget.now();
        if (!Number.isFinite(remaining))
          throw new Error("작업 시간 예산이 올바르지 않다");
        if (remaining < shardBudget) {
          process.stdout.write(
            `다음 조각을 시작할 작업 시간 부족: ${Math.max(0, Math.floor(remaining / 1000))}초\n`,
          );
          write("budget");
          return;
        }
      }
      const shard = `${index}/${parts}`;
      process.stdout.write(`변경분 변이 조각 ${shard}\n`);
      const status = run(base, shard);
      if (status === 124) {
        if (chunk > 1 && parts < maximumParts) {
          retry = true;
          break;
        }
        write("deadline");
        return;
      }
      if (status !== 0) throw new Error(`변이 조각 ${shard} 실패`);
      const catalog = read(`${output}/catalog.json`);
      const planned =
        catalog.length === 0 ? [] : read(`${output}/mutants.out/mutants.json`);
      const outcomes =
        catalog.length === 0
          ? undefined
          : read(`${output}/mutants.out/outcomes.json`);
      verifyShard(catalog, selected, planned, outcomes, index, parts);
    }
    if (!retry) {
      write("passed");
      return;
    }
    parts *= 2;
  }
};

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    const wallTime = Date.now();
    const monotonicTime = performance.now();
    runShards(
      process.env.BASE,
      (base, shard) =>
        spawnSync("just", ["mutants-diff-shard", base, shard], {
          stdio: "inherit",
        }).status,
      readJson,
      writeStatus,
      (base) =>
        spawnSync("just", ["mutants-diff-plan", base], { stdio: "inherit" })
          .status,
      {
        now: () => performance.now(),
        deadline:
          monotonicTime +
          jobDeadline(process.env.JOB_STARTED_AT, wallTime) -
          wallTime,
      },
    );
  } catch (error) {
    process.stderr.write(`diff-mutants: ${error.message}\n`);
    process.exitCode = 1;
  }
}
