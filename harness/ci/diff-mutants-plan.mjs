import { spawnSync } from "node:child_process";
import {
  appendFileSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";

export const MUTANTS_PER_SHARD = 20;
export const MAX_SHARDS = 8;
export const CATALOG_FILE = "target/mutants/catalog.json";
export const PLAN_FILE = "target/mutants/diff-plan.json";
export const EXCLUSIONS_FILE = "harness/ci/linux/exclusions.json";

const COMMIT = /^[0-9a-f]{40}$/u;

const SOURCE_PACKAGES = [
  ["crates/domain/src/", "domain"],
  ["crates/application/src/", "application"],
  ["crates/order-execution/src/", "order-execution"],
  ["crates/adapters/store-sqlite/src/", "store-sqlite"],
  ["crates/mcp-server/src/", "mcp-server"],
];

export const validCatalog = (catalog) =>
  Array.isArray(catalog) &&
  catalog.every(
    (entry) =>
      typeof entry?.name === "string" &&
      entry.name.length > 0 &&
      typeof entry.file === "string" &&
      entry.file.length > 0,
  );

export const packageOfSource = (file) => {
  if (typeof file !== "string" || file.length === 0) {
    throw new Error("변이 대상 파일 경로가 비어 있다");
  }
  const matched = SOURCE_PACKAGES.find(([prefix]) => file.startsWith(prefix));
  if (matched === undefined) {
    throw new Error(`변이 대상 플랫폼을 판정할 수 없는 파일: ${file}`);
  }
  return matched[1];
};

export const excludedCrateSet = (exclusions) => {
  if (
    typeof exclusions !== "object" ||
    exclusions === null ||
    !Array.isArray(exclusions.crates) ||
    !exclusions.crates.every(
      (entry) =>
        typeof entry?.name === "string" && entry.name.trim().length > 0,
    )
  ) {
    throw new Error(`${EXCLUSIONS_FILE}: crates 배열 형식이 올바르지 않다`);
  }
  return new Set(exclusions.crates.map((entry) => entry.name));
};

export const platformOfSource = (file, excluded) =>
  excluded.has(packageOfSource(file)) ? "macos" : "linux";

export const planShards = (count) => {
  if (!Number.isSafeInteger(count) || count < 0) {
    throw new Error(`잘못된 변이 수: ${count}`);
  }
  if (count === 0) {
    return [];
  }
  const total = Math.min(
    MAX_SHARDS,
    Math.max(1, Math.ceil(count / MUTANTS_PER_SHARD)),
  );
  return Array.from({ length: total }, (_, index) => `${index}/${total}`);
};

export const partitionCatalog = (catalog, exclusions) => {
  if (!validCatalog(catalog)) {
    throw new Error("변경분 변이 목록이 올바르지 않다");
  }
  const excluded = excludedCrateSet(exclusions);
  const linux = [];
  const macos = [];
  for (const entry of catalog) {
    if (platformOfSource(entry.file, excluded) === "macos") {
      macos.push(entry);
    } else {
      linux.push(entry);
    }
  }
  return { linux, macos };
};

export const buildPlan = (base, catalog, exclusions) => {
  if (typeof base !== "string" || !COMMIT.test(base)) {
    throw new Error("변이 기준 40자리 SHA가 필요하다");
  }
  const { linux, macos } = partitionCatalog(catalog, exclusions);
  const linuxShards = planShards(linux.length);
  const macosShards = planShards(macos.length);
  return {
    base,
    total: catalog.length,
    linuxCount: linux.length,
    macosCount: macos.length,
    linuxShards,
    macosShards,
    runLinux: linux.length > 0 ? "yes" : "no",
    runMacos: macos.length > 0 ? "yes" : "no",
  };
};

export const formatPlanSummary = (plan) =>
  [
    "## 변경분 변이 계획",
    "",
    `- 기준 커밋: \`${plan.base}\``,
    `- 전체 변이: ${plan.total}개 (Linux ${plan.linuxCount}개 / macOS ${plan.macosCount}개)`,
    `- Linux 조각 (${plan.linuxShards.length}개): ${plan.linuxShards.length === 0 ? "없음" : plan.linuxShards.join(", ")}`,
    `- macOS 조각 (${plan.macosShards.length}개): ${plan.macosShards.length === 0 ? "없음" : plan.macosShards.join(", ")}`,
    "",
  ].join("\n");

export const planOutputs = (plan) => ({
  base: plan.base,
  total: String(plan.total),
  linux_count: String(plan.linuxCount),
  macos_count: String(plan.macosCount),
  linux_shards: JSON.stringify(plan.linuxShards),
  macos_shards: JSON.stringify(plan.macosShards),
  run_linux: plan.runLinux,
  run_macos: plan.runMacos,
});

export const runPlan = (root, base, deps = {}) => {
  if (typeof base !== "string" || !COMMIT.test(base)) {
    throw new Error("변이 기준 40자리 SHA가 필요하다");
  }
  const run =
    deps.run ??
    ((commit) =>
      spawnSync("just", ["mutants-diff-plan-platform", commit, "all"], {
        cwd: root,
        stdio: "inherit",
      }).status);
  const readJson =
    deps.readJson ??
    ((relative) => JSON.parse(readFileSync(join(root, relative), "utf8")));
  const writeJson =
    deps.writeJson ??
    ((relative, value) => {
      const destination = join(root, relative);
      mkdirSync(dirname(destination), { recursive: true });
      writeFileSync(destination, `${JSON.stringify(value, undefined, 2)}\n`);
    });
  const emit =
    deps.emit ??
    ((outputs) => {
      for (const [key, value] of Object.entries(outputs)) {
        process.stdout.write(`${key}=${value}\n`);
        if (
          typeof process.env.GITHUB_OUTPUT === "string" &&
          process.env.GITHUB_OUTPUT.length > 0
        ) {
          appendFileSync(process.env.GITHUB_OUTPUT, `${key}=${value}\n`);
        }
      }
    });
  const summarize =
    deps.summarize ??
    ((text) => {
      process.stdout.write(text);
      if (
        typeof process.env.GITHUB_STEP_SUMMARY === "string" &&
        process.env.GITHUB_STEP_SUMMARY.length > 0
      ) {
        appendFileSync(process.env.GITHUB_STEP_SUMMARY, text);
      }
    });
  if (run(base) !== 0) {
    throw new Error("변경분 변이 계획 실패");
  }
  const catalog = readJson(CATALOG_FILE);
  const exclusions = readJson(EXCLUSIONS_FILE);
  const plan = buildPlan(base, catalog, exclusions);
  writeJson(PLAN_FILE, plan);
  emit(planOutputs(plan));
  summarize(formatPlanSummary(plan));
  return plan;
};

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    runPlan(process.cwd(), process.env.BASE ?? "");
  } catch (error) {
    process.stderr.write(
      `diff-mutants-plan: ${error instanceof Error ? error.message : String(error)}\n`,
    );
    process.exitCode = 1;
  }
}
