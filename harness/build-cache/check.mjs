import { readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dumpJustfile, renderLine } from "../check-list/list.mjs";

export const POLICY_FILE = "harness/build-cache/policy.json";
const WRAPPER_SH = "harness/build-cache/rustc-wrapper.sh";
const WRAPPER_MJS = "harness/build-cache/rustc-wrapper.mjs";
const PRUNE_MJS = "harness/build-cache/prune.mjs";

const REQUIRED_ISOLATED_RUST_FILES = [
  "crates/xtask/src/coverage_check.rs",
  "crates/xtask/src/mutants_check.rs",
  "crates/xtask/src/mutants_check/catalog.rs",
];

const USER_HOME_PATH = /(?:\/Users\/|\/home\/)[A-Za-z0-9._-]+/u;

export const policyProblems = (policy) => {
  const problems = [];
  if (policy?.schemaVersion !== 1) {
    problems.push(`${POLICY_FILE}: schemaVersion은 1이어야 한다`);
  }
  if (policy?.cacheSubdir !== "registry/stock-copilot-rustc-cache") {
    problems.push(
      `${POLICY_FILE}: cacheSubdir는 registry/stock-copilot-rustc-cache여야 한다`,
    );
  }
  if (
    !Number.isSafeInteger(policy?.defaultMaxMegabytes) ||
    policy.defaultMaxMegabytes < 512 ||
    policy.defaultMaxMegabytes > 32768
  ) {
    problems.push(
      `${POLICY_FILE}: defaultMaxMegabytes는 512..=32768 정수여야 한다`,
    );
  }
  if (
    typeof policy?.pruneTargetRatio !== "number" ||
    !(policy.pruneTargetRatio > 0 && policy.pruneTargetRatio < 1)
  ) {
    problems.push(`${POLICY_FILE}: pruneTargetRatio는 0과 1 사이여야 한다`);
  }
  if (
    !Number.isSafeInteger(policy?.minEntryAgeSeconds) ||
    policy.minEntryAgeSeconds < 0
  ) {
    problems.push(`${POLICY_FILE}: minEntryAgeSeconds는 0 이상 정수여야 한다`);
  }
  if (
    !Number.isSafeInteger(policy?.staleTempAgeSeconds) ||
    policy.staleTempAgeSeconds < 60
  ) {
    problems.push(
      `${POLICY_FILE}: staleTempAgeSeconds는 60 이상 정수여야 한다`,
    );
  }
  if (
    !Number.isSafeInteger(policy?.maxOutDirFiles) ||
    policy.maxOutDirFiles < 1 ||
    !Number.isSafeInteger(policy?.maxOutDirBytes) ||
    policy.maxOutDirBytes < 1024 ||
    !Number.isSafeInteger(policy?.maxNativeDirBytes) ||
    policy.maxNativeDirBytes < 1024
  ) {
    problems.push(
      `${POLICY_FILE}: maxOutDirFiles·maxOutDirBytes·maxNativeDirBytes 상한이 올바르지 않다`,
    );
  }
  return problems;
};

const wrapperProblems = (root) => {
  const problems = [];
  const shPath = join(root, WRAPPER_SH);
  const mode = statSync(shPath).mode;
  if ((mode & 0o111) === 0) {
    problems.push(`${WRAPPER_SH}: 실행 권한(+x)이 없다`);
  }
  const shText = readFileSync(shPath, "utf8");
  for (const required of [
    "CI",
    "GITHUB_ACTIONS",
    "STOCK_COPILOT_BUILD_CACHE",
    "CARGO_PRIMARY_PACKAGE",
    "LLVM_PROFILE_FILE",
    "instrument-coverage",
    "incremental=",
    "*/registry/src/*.rs",
  ]) {
    if (!shText.includes(required)) {
      problems.push(`${WRAPPER_SH}: 필수 보호 분기 '${required}'가 없다`);
    }
  }
  for (const file of [WRAPPER_SH, WRAPPER_MJS, PRUNE_MJS, POLICY_FILE]) {
    const text = readFileSync(join(root, file), "utf8");
    if (USER_HOME_PATH.test(text)) {
      problems.push(`${file}: 사용자별 절대 홈 경로가 들어 있다`);
    }
  }
  return problems;
};

const justfileCacheProblems = (root) => {
  const text = readFileSync(join(root, "justfile"), "utf8");
  const problems = [];
  if (USER_HOME_PATH.test(text)) {
    problems.push("justfile: 사용자별 절대 홈 경로가 들어 있다");
  }
  const dump = dumpJustfile(join(root, "justfile"));
  if (dump.problem !== undefined) {
    return [`justfile을 읽지 못했다: ${dump.problem}`];
  }
  const unexports = Array.isArray(dump.unexports) ? dump.unexports : [];
  for (const required of [
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
  ]) {
    if (!unexports.includes(required)) {
      problems.push(`justfile: unexport ${required}가 없다`);
    }
  }
  const wrapperAssignment = JSON.stringify(dump.assignments?.RUSTC_WRAPPER);
  if (dump.assignments?.RUSTC_WRAPPER?.export !== true) {
    problems.push("justfile: RUSTC_WRAPPER export 선언이 없다");
  }
  for (const token of [
    "CI",
    "GITHUB_ACTIONS",
    "STOCK_COPILOT_BUILD_CACHE",
    "harness/build-cache/rustc-wrapper.sh",
  ]) {
    if (!wrapperAssignment.includes(token)) {
      problems.push(`justfile: RUSTC_WRAPPER 분기에 '${token}'가 없다`);
    }
  }
  const workspaceWrapper = dump.assignments?.RUSTC_WORKSPACE_WRAPPER;
  if (workspaceWrapper?.export !== true || workspaceWrapper?.value !== "") {
    problems.push('justfile: RUSTC_WORKSPACE_WRAPPER는 ""로 고정해야 한다');
  }
  const recipes = dump.recipes ?? {};
  const recipeLines = (name) => (recipes[name]?.body ?? []).map(renderLine);
  if (
    !recipeLines("clean-worktree").includes("cargo clean --locked") ||
    !recipeLines("clean-worktree").includes("cargo llvm-cov clean --locked") ||
    !recipeLines("clean-worktree").includes(
      "node harness/build-cache/prune.mjs",
    )
  ) {
    problems.push(
      "justfile: clean-worktree는 cargo clean, llvm-cov clean, cache prune을 모두 실행해야 한다",
    );
  }
  if (
    !recipeLines("cache-prune").includes("node harness/build-cache/prune.mjs")
  ) {
    problems.push("justfile: cache-prune 레시피가 없다");
  }
  if (
    !recipeLines("cache-clear").includes(
      "node harness/build-cache/prune.mjs --clear",
    )
  ) {
    problems.push("justfile: cache-clear 레시피가 없다");
  }
  if (
    !recipeLines("cache-stats").includes(
      "node harness/build-cache/prune.mjs --stats",
    )
  ) {
    problems.push("justfile: cache-stats 레시피가 없다");
  }
  return problems;
};

const isolationProblems = (root) => {
  const problems = [];
  for (const file of REQUIRED_ISOLATED_RUST_FILES) {
    const production =
      readFileSync(join(root, file), "utf8").split(
        /#\[cfg\(test\)\]\s*mod\s+[A-Za-z0-9_]+\s*\{/u,
      )[0] ?? "";
    if (
      !production.includes('"STOCK_COPILOT_BUILD_CACHE", "off"') ||
      !production.includes('"RUSTC_WRAPPER", ""')
    ) {
      problems.push(
        `${file}: 커버리지·변이 실행은 STOCK_COPILOT_BUILD_CACHE=off와 RUSTC_WRAPPER=""로 공유 캐시를 꺼야 한다`,
      );
    }
  }
  return problems;
};

export const buildCacheProblems = (root) => {
  let policy;
  try {
    policy = JSON.parse(readFileSync(join(root, POLICY_FILE), "utf8"));
  } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }
  return [
    ...policyProblems(policy),
    ...wrapperProblems(root),
    ...justfileCacheProblems(root),
    ...isolationProblems(root),
  ];
};

const repoRoot = resolve(fileURLToPath(import.meta.url), "..", "..", "..");

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = resolve(process.argv[2] ?? repoRoot);
  const problems = buildCacheProblems(root);
  problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
  process.stdout.write(`build-cache-policy: ${problems.length} failure(s)\n`);
  process.exitCode = problems.length === 0 ? 0 : 1;
}
