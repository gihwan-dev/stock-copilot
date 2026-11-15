import { spawnSync } from "node:child_process";
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export const shardsFor = (count) => {
  if (!Number.isSafeInteger(count) || count < 0)
    throw new Error("잘못된 변이 수");
  const total = Math.min(8, Math.max(1, Math.ceil(count / 100)));
  return Array.from({ length: total }, (_, index) => `${index}/${total}`);
};
export const sliceFor = (catalog, shard) => {
  if (typeof shard !== "string" || !/^[0-7]\/[1-8]$/u.test(shard))
    throw new Error("잘못된 일일 조각");
  const [index, total] = shard.split("/").map(Number);
  if (index >= total) throw new Error("잘못된 일일 조각 범위");
  const chunk = Math.ceil(catalog.length / total);
  return catalog.slice(index * chunk, (index + 1) * chunk);
};
export const runAnchor = (event, createdAt) => {
  if (
    typeof createdAt !== "string" ||
    !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u.test(createdAt)
  )
    throw new Error("실행 생성 시각 형식이 아니다");
  const date = new Date(createdAt);
  if (!Number.isFinite(date.getTime()))
    throw new Error("실행 생성 시각이 없다");
  if (event === "workflow_dispatch") return date;
  if (event !== "schedule") throw new Error("일일 실행 이벤트가 아니다");
  const anchor = new Date(
    Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate(), 18),
  );
  if (anchor > date) anchor.setUTCDate(anchor.getUTCDate() - 1);
  return anchor;
};
const createdAt = () => {
  const repository = process.env.GITHUB_REPOSITORY;
  const runId = process.env.GITHUB_RUN_ID;
  if (
    !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/u.test(repository) ||
    !/^\d+$/u.test(runId)
  )
    throw new Error("실행 식별자가 없다");
  const result = spawnSync(
    "gh",
    ["api", `repos/${repository}/actions/runs/${runId}`, "--jq", ".created_at"],
    { encoding: "utf8" },
  );
  if (result.status !== 0)
    throw new Error(`실행 생성 시각 조회 실패: ${result.stderr}`);
  return result.stdout.trim();
};
export const preparePlan = (root, range) => {
  const anchor = runAnchor(process.env.GITHUB_EVENT_NAME, createdAt());
  const base = range(root, anchor);
  const run = spawnSync(
    "cargo",
    [
      "run",
      "--locked",
      "--quiet",
      "--package",
      "xtask",
      "--",
      "mutants",
      "--daily-plan",
      base,
    ],
    { cwd: root, stdio: "inherit" },
  );
  if (run.status !== 0) throw new Error("일일 변이 계획 실패");
  const catalog = JSON.parse(
    readFileSync(join(root, "target/mutants/catalog.json"), "utf8"),
  );
  if (!Array.isArray(catalog)) throw new Error("계획 목록이 아니다");
  const shards = shardsFor(catalog.length);
  writeFileSync(
    join(root, "target/mutants/plan.json"),
    JSON.stringify({ shards, anchor: anchor.toISOString() }),
  );
  for (const [key, value] of Object.entries({
    base,
    shards: JSON.stringify(shards),
    count: catalog.length,
    anchor: anchor.toISOString(),
  })) {
    process.stdout.write(`${key}=${value}\n`);
    if (process.env.GITHUB_OUTPUT !== undefined)
      appendFileSync(process.env.GITHUB_OUTPUT, `${key}=${value}\n`);
  }
};
