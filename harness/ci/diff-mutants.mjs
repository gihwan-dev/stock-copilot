import { spawnSync } from "node:child_process";
import { appendFileSync, readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const parts = 4;
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

export const verifyShard = (catalog, previous, planned, outcomes, index) => {
  if (
    !validCatalog(catalog) ||
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

export const runShards = (base, run, read, write) => {
  if (typeof base !== "string" || !/^[0-9a-f]{40}$/u.test(base))
    throw new Error("변이 기준 SHA가 없다");
  let previous;
  for (let index = 0; index < parts; index += 1) {
    const shard = `${index}/${parts}`;
    process.stdout.write(`변경분 변이 조각 ${shard}\n`);
    const status = run(base, shard);
    if (status === 124) {
      write("deadline");
      return;
    }
    if (status !== 0) throw new Error(`변이 조각 ${shard} 실패`);
    const catalog = read(`${output}/catalog.json`);
    const chunk = Math.ceil(catalog.length / parts);
    const empty = catalog.length === 0 || index * chunk >= catalog.length;
    const planned =
      catalog.length === 0 ? [] : read(`${output}/mutants.out/mutants.json`);
    const outcomes = empty
      ? undefined
      : read(`${output}/mutants.out/outcomes.json`);
    verifyShard(catalog, previous, planned, outcomes, index);
    previous = catalog;
  }
  write("passed");
};

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    runShards(
      process.env.BASE,
      (base, shard) =>
        spawnSync("just", ["mutants-diff-shard", base, shard], {
          stdio: "inherit",
        }).status,
      readJson,
      writeStatus,
    );
  } catch (error) {
    process.stderr.write(`diff-mutants: ${error.message}\n`);
    process.exitCode = 1;
  }
}
