import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dumpJustfile, renderLine } from "../check-list/list.mjs";

export const POLICY_FILE = "harness/proptest/policy.toml";
export const MIN_WORKING_CASES = 256;
export const MIN_FULL_CASES = 4096;

const FULL_RECIPES = ["check-audited", "coverage-check", "linux-test"];
const WORKFLOWS = [".github/workflows/ci.yml", ".github/workflows/nightly.yml"];

export const parsePolicy = (text) => {
  const entries = new Map();
  for (const raw of text.split(/\r?\n/u)) {
    const line = raw.trim();
    if (line === "") {
      continue;
    }
    const match = /^([a-z_]+)\s*=\s*(\d+)$/u.exec(line);
    if (match === null) {
      throw new Error(`${POLICY_FILE}: 잘못된 정책 줄: ${line}`);
    }
    if (entries.has(match[1])) {
      throw new Error(`${POLICY_FILE}: 중복 키: ${match[1]}`);
    }
    entries.set(match[1], Number(match[2]));
  }
  const keys = [...entries.keys()].sort();
  if (
    keys.length !== 2 ||
    keys[0] !== "full_cases" ||
    keys[1] !== "working_cases"
  ) {
    throw new Error(
      `${POLICY_FILE}: working_cases와 full_cases 두 키만 있어야 한다`,
    );
  }
  const workingCases = entries.get("working_cases");
  const fullCases = entries.get("full_cases");
  if (!Number.isSafeInteger(workingCases) || !Number.isSafeInteger(fullCases)) {
    throw new Error(`${POLICY_FILE}: 횟수는 안전한 정수여야 한다`);
  }
  return { workingCases, fullCases };
};

export const policyProblems = (policy) => {
  const problems = [];
  if (policy.workingCases < MIN_WORKING_CASES) {
    problems.push(
      `${POLICY_FILE}: working_cases(${policy.workingCases})는 ${MIN_WORKING_CASES} 미만일 수 없다`,
    );
  }
  if (policy.fullCases < MIN_FULL_CASES) {
    problems.push(
      `${POLICY_FILE}: full_cases(${policy.fullCases})는 ${MIN_FULL_CASES} 미만일 수 없다`,
    );
  }
  if (policy.workingCases > policy.fullCases) {
    problems.push(
      `${POLICY_FILE}: working_cases(${policy.workingCases})는 full_cases(${policy.fullCases})를 넘을 수 없다`,
    );
  }
  return problems;
};

const envAttribute = (recipe, name) => {
  const attributes = recipe?.attributes ?? [];
  for (const entry of attributes) {
    if (
      typeof entry === "object" &&
      entry !== null &&
      Array.isArray(entry.env) &&
      entry.env[0] === name
    ) {
      return entry.env[1];
    }
  }
  return undefined;
};

const testkitProblems = (text, policy) => {
  const problems = [];
  const casesMatch =
    /fn cases_for\([^)]*\)\s*->\s*u32\s*\{[\s\S]*?None\s*=>\s*(\d+),[\s\S]*?Some\(_\)\s*=>\s*(\d+),/u.exec(
      text,
    );
  if (casesMatch === null) {
    problems.push(
      "crates/testkit/src/lib.rs: cases_for의 working/full 분기를 찾지 못했다",
    );
    return problems;
  }
  if (Number(casesMatch[1]) !== policy.workingCases) {
    problems.push(
      `crates/testkit/src/lib.rs: cases_for(None) 값(${casesMatch[1]})이 ${policy.workingCases}와 다르다`,
    );
  }
  if (Number(casesMatch[2]) !== policy.fullCases) {
    problems.push(
      `crates/testkit/src/lib.rs: cases_for(Some(_)) 값(${casesMatch[2]})이 ${policy.fullCases}와 다르다`,
    );
  }
  return problems;
};

const justfileProblems = (dump, policy) => {
  if (dump.problem !== undefined) {
    return [`justfile을 읽지 못했다: ${dump.problem}`];
  }
  const problems = [];
  const unexports = Array.isArray(dump.unexports) ? dump.unexports : [];
  for (const required of ["STOCK_COPILOT_PROPTEST_CASES", "PROPTEST_CASES"]) {
    if (!unexports.includes(required)) {
      problems.push(`justfile: unexport ${required}가 없다`);
    }
  }
  if (unexports.includes("PROPTEST_RNG_SEED")) {
    problems.push(
      "justfile: PROPTEST_RNG_SEED를 unexport하면 씨앗 고정이 깨진다",
    );
  }
  const recipes = dump.recipes ?? {};
  for (const name of FULL_RECIPES) {
    const raw = envAttribute(recipes[name], "STOCK_COPILOT_PROPTEST_CASES");
    const parsed = raw === undefined ? NaN : Number(raw);
    if (!/^\d+$/u.test(raw ?? "") || parsed < policy.fullCases) {
      problems.push(
        `justfile: ${name}의 STOCK_COPILOT_PROPTEST_CASES(${raw ?? "없음"})는 ${policy.fullCases} 이상이어야 한다`,
      );
    }
  }
  for (const name of ["check-fast", "test-fast"]) {
    const recipe = recipes[name];
    if (recipe === undefined) {
      problems.push(`justfile: ${name} 레시피가 없다`);
      continue;
    }
    if (
      envAttribute(recipe, "STOCK_COPILOT_PROPTEST_CASES") !== undefined ||
      envAttribute(recipe, "PROPTEST_CASES") !== undefined
    ) {
      problems.push(
        `justfile: ${name}에 속성 횟수 환경 고정이 붙어 있으면 안 된다`,
      );
    }
    if (envAttribute(recipe, "PROPTEST_RNG_SEED") !== undefined) {
      problems.push(`justfile: ${name}은 PROPTEST_RNG_SEED를 고정할 수 없다`);
    }
  }
  const testFastBody = (recipes["test-fast"]?.body ?? []).map(renderLine);
  if (!testFastBody.includes('cargo nextest run --locked "$@"')) {
    problems.push(
      'justfile: test-fast는 cargo nextest run --locked "$@"를 실행해야 한다',
    );
  }
  return problems;
};

const fastCheckProblems = (text) => {
  const production =
    text.split(/#\[cfg\(test\)\]\s*mod\s+[A-Za-z0-9_]+\s*\{/u)[0] ?? "";
  const problems = [];
  if (
    !production.includes('"STOCK_COPILOT_PROPTEST_CASES"') ||
    !production.includes('"PROPTEST_CASES"') ||
    !production.includes(
      'execute_with_removed_env(root, "cargo", &tests, &WORKING_PROPTEST_ENV)',
    )
  ) {
    problems.push(
      "crates/xtask/src/fast_check.rs: 빠른 경로에서 STOCK_COPILOT_PROPTEST_CASES와 PROPTEST_CASES를 제거해야 한다",
    );
  }
  if (production.includes('"PROPTEST_RNG_SEED"')) {
    problems.push(
      "crates/xtask/src/fast_check.rs: PROPTEST_RNG_SEED를 제거하면 씨앗 고정이 깨진다",
    );
  }
  if (!production.includes('return just_step("check");')) {
    problems.push(
      "crates/xtask/src/fast_check.rs: 승격 경로는 전체 check 레시피(4096회)를 실행해야 한다",
    );
  }
  return problems;
};

const workflowProblems = (path, workflow) => {
  const problems = [];
  const raw = JSON.stringify(workflow);
  if (
    /\bSTOCK_COPILOT_PROPTEST_CASES\b/u.test(raw) ||
    /\bPROPTEST_CASES\b/u.test(raw)
  ) {
    problems.push(
      `${path}: 워크플로가 STOCK_COPILOT_PROPTEST_CASES 또는 PROPTEST_CASES를 덮어쓴다`,
    );
  }
  if (/\bcheck-fast\b/u.test(raw) || /\btest-fast\b/u.test(raw)) {
    problems.push(
      `${path}: CI 워크플로는 작업 중 검사(check-fast/test-fast)를 실행할 수 없다`,
    );
  }
  const jobs = workflow.jobs ?? {};
  const hasCommand = (jobName, pattern) =>
    (jobs[jobName]?.steps ?? []).some((step) => pattern.test(step.run ?? ""));
  if (path.endsWith("/ci.yml")) {
    if (!hasCommand("linux", /\bjust linux-test\b/u)) {
      problems.push(`${path}: linux 작업에 just linux-test가 없다`);
    }
    for (const macJob of ["macos", "macos-branch"]) {
      if (!hasCommand(macJob, /^just base="\$BASE" check$/u)) {
        problems.push(`${path}: ${macJob} 작업에 전체 just check가 없다`);
      }
    }
  }
  if (path.endsWith("/nightly.yml")) {
    if (!hasCommand("linux", /\bjust linux-test\b/u)) {
      problems.push(`${path}: linux 작업에 just linux-test가 없다`);
    }
    if (!hasCommand("macos", /^just base=HEAD check$/u)) {
      problems.push(`${path}: macos 작업에 전체 just check가 없다`);
    }
  }
  return problems;
};

export const proptestProblems = (root) => {
  let policy;
  try {
    policy = parsePolicy(readFileSync(join(root, POLICY_FILE), "utf8"));
  } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }
  const problems = [...policyProblems(policy)];
  problems.push(
    ...testkitProblems(
      readFileSync(join(root, "crates/testkit/src/lib.rs"), "utf8"),
      policy,
    ),
  );
  problems.push(
    ...justfileProblems(dumpJustfile(join(root, "justfile")), policy),
  );
  problems.push(
    ...fastCheckProblems(
      readFileSync(join(root, "crates/xtask/src/fast_check.rs"), "utf8"),
    ),
  );
  for (const path of WORKFLOWS) {
    problems.push(
      ...workflowProblems(
        path,
        JSON.parse(readFileSync(join(root, path), "utf8")),
      ),
    );
  }
  return problems;
};

const repoRoot = resolve(fileURLToPath(import.meta.url), "..", "..", "..");

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = resolve(process.argv[2] ?? repoRoot);
  const problems = proptestProblems(root);
  problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
  process.stdout.write(`proptest-policy: ${problems.length} failure(s)\n`);
  process.exitCode = problems.length === 0 ? 0 : 1;
}
