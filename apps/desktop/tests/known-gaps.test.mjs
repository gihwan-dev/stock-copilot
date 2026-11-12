import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { checkSteps } from "../../../harness/check-list/list.mjs";
import { knownGapProblems } from "../../../harness/eslint-violations/verdict.mjs";
import { gapTable, knownGaps } from "../../../harness/known-gaps/list.mjs";
import { repoRoot } from "./paths.mjs";

const readJson = (path) =>
  JSON.parse(readFileSync(join(repoRoot, path), "utf8"));

const eslintCases = readJson("harness/eslint-violations/cases.json").cases;

test("the known gap list gathers every ESLint gap example, every hook gap case and every ADR gap item", () => {
  const gaps = knownGaps(repoRoot);
  const eslint = Object.entries(eslintCases)
    .filter(([, spec]) => spec.expect === "KNOWN_GAP")
    .map(([file]) => `harness/eslint-violations/${file}`);
  assert.ok(eslint.length >= 7);
  assert.deepEqual(
    gaps.filter((gap) => gap.source === "eslint").map((gap) => gap.location),
    eslint,
  );
  const hookGaps = [
    "harness/claude-hooks/bash-cases.json",
    "harness/claude-hooks/edit-cases.json",
    "harness/claude-hooks/agent-cases.json",
  ].flatMap((path) => readJson(path).filter((item) => item.gap !== undefined));
  assert.ok(hookGaps.length >= 6);
  assert.equal(
    gaps.filter((gap) => gap.source === "hook").length,
    hookGaps.length,
  );
  const adr = gaps.filter((gap) => gap.source === "adr");
  [
    "docs/adr/0005-rust-lints-and-third-party-allowlist.md",
    "docs/adr/0017-claude-code-edit-and-shell-hooks.md",
    "docs/adr/0018-subagent-hooks-and-strict-sandbox.md",
  ].forEach((path) =>
    assert.ok(
      adr.some((gap) => gap.location.startsWith(`${path}:`)),
      path,
    ),
  );
  assert.ok(adr.some((gap) => gap.text.startsWith("(b) `PATH`에 놓인 가짜")));
  assert.ok(
    adr.some((gap) => gap.text.startsWith("대소문자를 구분하는 파일 시스템")),
  );
});

test("every ESLint known gap carries the exact reports it gets today", () => {
  const gaps = Object.entries(eslintCases).filter(
    ([, spec]) => spec.expect === "KNOWN_GAP",
  );
  assert.deepEqual(
    gaps
      .filter(([, spec]) => !Array.isArray(spec.reports))
      .map(([file]) => file),
    [],
  );
});

test("a known ESLint gap that gets a new report or loses one fails instead of passing blurred", () => {
  const spec = { row: "React", expect: "KNOWN_GAP", reports: ["a", "b"] };
  assert.deepEqual(knownGapProblems(spec, ["a", "b"]), []);
  assert.deepEqual(knownGapProblems(spec, ["a", "b", "c"]), [
    "known gap changed: unexpected c",
  ]);
  assert.deepEqual(knownGapProblems(spec, ["a"]), [
    "known gap changed: missing b",
  ]);
  assert.deepEqual(
    knownGapProblems({ row: "React", expect: "KNOWN_GAP" }, []),
    ["known gap without the exact reports it gets today"],
  );
});

test("just known-gaps prints the list read-only and is not part of just check", () => {
  const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");
  assert.ok(
    justfile.includes("known-gaps:\n    node harness/known-gaps/list.mjs\n"),
  );
  assert.ok(
    checkSteps(justfile).every(
      (step) => !(step.command ?? "").includes("known-gaps"),
    ),
  );
  const run = spawnSync(
    process.execPath,
    [join(repoRoot, "harness/known-gaps/list.mjs")],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  const gaps = knownGaps(repoRoot);
  assert.equal(run.stdout, `${gapTable(gaps)}\n`);
  assert.match(
    run.stdout,
    /known-gaps: \d+건 \(eslint 7건, hook 6건, adr \d+건\)/u,
  );
});
