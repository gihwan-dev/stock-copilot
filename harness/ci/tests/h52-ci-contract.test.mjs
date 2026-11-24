import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { join } from "node:path";
import { repoRoot } from "./signature-repo.mjs";

const ci = JSON.parse(
  readFileSync(join(repoRoot, ".github/workflows/ci.yml"), "utf8"),
);
const macos = ci.jobs["macos-branch"];

test("작업 브랜치 push도 macOS 전체 검사와 변경분 변이를 실행한다", () => {
  assert.equal(
    macos.if,
    "github.event_name == 'push' && github.ref != 'refs/heads/master'",
  );
  assert.equal(macos["runs-on"], "macos-15");
  assert.equal(macos["timeout-minutes"], 120);
  assert.equal(ci.jobs.macos["timeout-minutes"], 120);
  assert.ok(macos.steps.some((step) => step.run === 'just base="$BASE" check'));
  assert.ok(
    macos.steps.findIndex((step) => step.name === "branch checkout") <
      macos.steps.findIndex((step) => step.name === "just check"),
  );
  assert.equal(ci.jobs["diff-mutants-plan"].if, "github.event_name == 'push'");
  assert.ok(
    ci.jobs["diff-mutants-plan"].steps.some(
      (step) => step.run === "node harness/ci/diff-mutants-plan.mjs",
    ),
  );
  assert.equal(
    ci.jobs["diff-mutants-verdict"].if,
    "always() && github.event_name == 'push'",
  );
  assert.ok(
    ci.jobs["diff-mutants-verdict"].steps.some(
      (step) => step.run === "node harness/ci/diff-mutants-aggregate.mjs",
    ),
  );
});

test("nextest는 환경 변수가 제거되는 just 안에서도 동시에 네 개까지만 실행한다", () => {
  const config = readFileSync(join(repoRoot, ".config/nextest.toml"), "utf8");
  assert.match(config, /\[profile\.default\]\s*\ntest-threads = 4(?:\n|$)/u);
});
