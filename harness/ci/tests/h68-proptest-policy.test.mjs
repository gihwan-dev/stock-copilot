import { strict as assert } from "node:assert";
import {
  cpSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  MIN_FULL_CASES,
  MIN_WORKING_CASES,
  POLICY_FILE,
  parsePolicy,
  policyProblems,
  proptestProblems,
} from "../../proptest/check.mjs";
import { repoRoot, scratch } from "./signature-repo.mjs";

const copyProptestFixture = () => {
  const dir = scratch("h68-proptest-");
  for (const rel of [
    POLICY_FILE,
    "crates/testkit/src/lib.rs",
    "crates/xtask/src/fast_check.rs",
    "justfile",
    ".github/workflows/ci.yml",
    ".github/workflows/nightly.yml",
  ]) {
    const dst = join(dir, rel);
    mkdirSync(join(dst, ".."), { recursive: true });
    cpSync(join(repoRoot, rel), dst);
  }
  return dir;
};

test("H68 proptest policy enforces 256 working cases 4096 full cases and seed pinning", () => {
  const policy = parsePolicy(readFileSync(join(repoRoot, POLICY_FILE), "utf8"));
  assert.equal(policy.workingCases, MIN_WORKING_CASES);
  assert.equal(policy.fullCases, MIN_FULL_CASES);
  assert.deepEqual(policyProblems(policy), []);
  assert.deepEqual(proptestProblems(repoRoot), []);
});

test("H68 proptest policy rejects lowering working below 256 or full below 4096", () => {
  const lowWorking = copyProptestFixture();
  try {
    writeFileSync(
      join(lowWorking, POLICY_FILE),
      "working_cases = 255\nfull_cases = 4096\n",
    );
    assert.ok(
      proptestProblems(lowWorking).some((item) =>
        item.includes("256 미만일 수 없다"),
      ),
    );
  } finally {
    rmSync(lowWorking, { recursive: true, force: true });
  }

  const lowFull = copyProptestFixture();
  try {
    writeFileSync(
      join(lowFull, POLICY_FILE),
      "working_cases = 256\nfull_cases = 2048\n",
    );
    assert.ok(
      proptestProblems(lowFull).some((item) =>
        item.includes("4096 미만일 수 없다"),
      ),
    );
  } finally {
    rmSync(lowFull, { recursive: true, force: true });
  }

  const fullRecipeWeakened = copyProptestFixture();
  try {
    const justPath = join(fullRecipeWeakened, "justfile");
    writeFileSync(
      justPath,
      readFileSync(justPath, "utf8").replace(
        '[env("STOCK_COPILOT_PROPTEST_CASES", "4096")]\ncheck-audited:',
        '[env("STOCK_COPILOT_PROPTEST_CASES", "256")]\ncheck-audited:',
      ),
    );
    assert.ok(
      proptestProblems(fullRecipeWeakened).some((item) =>
        item.includes("check-audited"),
      ),
    );
  } finally {
    rmSync(fullRecipeWeakened, { recursive: true, force: true });
  }

  const linuxTestWeakened = copyProptestFixture();
  try {
    const justPath = join(linuxTestWeakened, "justfile");
    writeFileSync(
      justPath,
      readFileSync(justPath, "utf8").replace(
        '[env("STOCK_COPILOT_PROPTEST_CASES", "4096")]\nlinux-test:',
        '[env("STOCK_COPILOT_PROPTEST_CASES", "512")]\nlinux-test:',
      ),
    );
    assert.ok(
      proptestProblems(linuxTestWeakened).some((item) =>
        item.includes("linux-test"),
      ),
    );
  } finally {
    rmSync(linuxTestWeakened, { recursive: true, force: true });
  }

  const seedUnexported = copyProptestFixture();
  try {
    const justPath = join(seedUnexported, "justfile");
    writeFileSync(
      justPath,
      `unexport PROPTEST_RNG_SEED\n${readFileSync(justPath, "utf8")}`,
    );
    assert.ok(
      proptestProblems(seedUnexported).some((item) =>
        item.includes("PROPTEST_RNG_SEED"),
      ),
    );
  } finally {
    rmSync(seedUnexported, { recursive: true, force: true });
  }

  const ciUsesFast = copyProptestFixture();
  try {
    const ciPath = join(ciUsesFast, ".github/workflows/ci.yml");
    const ci = JSON.parse(readFileSync(ciPath, "utf8"));
    ci.jobs.macos.steps.find((step) => step.name === "just check").run =
      'just base="$BASE" check-fast';
    writeFileSync(ciPath, `${JSON.stringify(ci, undefined, 2)}\n`);
    assert.ok(
      proptestProblems(ciUsesFast).some((item) => item.includes("check-fast")),
    );
  } finally {
    rmSync(ciUsesFast, { recursive: true, force: true });
  }

  const testkitMismatch = copyProptestFixture();
  try {
    const libPath = join(testkitMismatch, "crates/testkit/src/lib.rs");
    writeFileSync(
      libPath,
      readFileSync(libPath, "utf8").replace("None => 256,", "None => 128,"),
    );
    assert.ok(
      proptestProblems(testkitMismatch).some((item) =>
        item.includes("cases_for(None)"),
      ),
    );
  } finally {
    rmSync(testkitMismatch, { recursive: true, force: true });
  }
});
