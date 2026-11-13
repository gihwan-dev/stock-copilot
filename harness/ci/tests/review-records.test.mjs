import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import {
  DONE_STATES,
  OPEN_STATES,
  PROGRESS_FILE,
  REVIEWS_DIRECTORY,
  SECTIONS,
  TABLE_HEADER,
  WAIVER_DECIDERS,
  WAIVER_REASON_MINIMUM,
  isDone,
  isKnownState,
  reviewProblems,
} from "../review-records/check.mjs";
import {
  childEnvironment,
  gitIn,
  repoRoot,
  scratch,
} from "./signature-repo.mjs";

const CASES_FILE = "harness/ci/review-records/cases.json";

const cases = JSON.parse(
  readFileSync(join(repoRoot, CASES_FILE), "utf8"),
).cases;

const fixture = {};

before(() => {
  fixture.directory = scratch("review-records-");
  gitIn(fixture.directory, ["init", "--quiet"]);
  gitIn(fixture.directory, [
    "commit",
    "--quiet",
    "--allow-empty",
    "-m",
    "base",
  ]);
  fixture.base = gitIn(fixture.directory, ["rev-parse", "--short=7", "HEAD"]);
  gitIn(fixture.directory, [
    "commit",
    "--quiet",
    "--allow-empty",
    "-m",
    "target",
  ]);
  fixture.commit = gitIn(fixture.directory, ["rev-parse", "HEAD"]);
});

after(() => rmSync(fixture.directory, { recursive: true, force: true }));

const filled = (text) =>
  text
    .replaceAll("{commit}", fixture.commit)
    .replaceAll("{base}", fixture.base);

const problemsOf = (entry) => {
  rmSync(join(fixture.directory, "docs"), { recursive: true, force: true });
  mkdirSync(join(fixture.directory, REVIEWS_DIRECTORY), { recursive: true });
  writeFileSync(join(fixture.directory, PROGRESS_FILE), filled(entry.progress));
  Object.entries(entry.reviews).forEach(([name, text]) =>
    writeFileSync(
      join(fixture.directory, REVIEWS_DIRECTORY, name),
      filled(text),
    ),
  );
  return reviewProblems(fixture.directory);
};

const allCasesMatch = () =>
  cases.every(
    (entry) =>
      JSON.stringify(problemsOf(entry)) === JSON.stringify(entry.problems),
  );

test("every review record case reports exactly its problems", () => {
  cases.forEach((entry) =>
    assert.deepEqual(problemsOf(entry), entry.problems, entry.name),
  );
});

test("the repository's completed tasks all have review records with the required sections", () => {
  assert.deepEqual(reviewProblems(repoRoot), []);
});

test("the review record check exits non-zero and names the row when a record is missing", () => {
  const entry = cases.find((candidate) => candidate.name === "missing-link");
  problemsOf(entry);
  const finished = spawnSync(
    "node",
    ["harness/ci/review-records/check.mjs", fixture.directory],
    { cwd: repoRoot, encoding: "utf8", env: childEnvironment() },
  );
  assert.equal(finished.status, 1);
  assert.match(finished.stdout, /FAIL docs\/progress\.md:5 H-02/u);
});

test("states are an exact list and only the done ones need a record", () => {
  assert.deepEqual(TABLE_HEADER, ["작업", "상태", "대상 커밋", "리뷰 기록"]);
  assert.deepEqual(DONE_STATES, ["완료", "일부 완료"]);
  assert.deepEqual(OPEN_STATES, ["해결"]);
  assert.deepEqual(WAIVER_DECIDERS, ["사용자"]);
  assert.equal(WAIVER_REASON_MINIMUM, 20);
  ["완료", "일부 완료", "완료 (3회차까지)", "완료 (12회차까지)"].forEach(
    (state) => assert.equal(isDone(state), true, state),
  );
  [
    "해결",
    "완료 ",
    " 완료",
    "완료됨",
    "**완료**",
    "완료 (회차까지)",
    "완료 (3회차)",
    "일부 완료됨",
    "진행 중",
    "",
  ].forEach((state) => assert.equal(isDone(state), false, state));
  assert.equal(isKnownState("해결"), true);
  assert.equal(isKnownState("진행 중"), false);
  assert.deepEqual(
    SECTIONS.map((section) => section.name),
    ["커밋 표", "리뷰 표", "측정 표"],
  );
});

test("dropping any required table column, filled cell or commit cell makes some case pass that should fail", () => {
  const surviving = [];
  SECTIONS.forEach((section) => {
    ["table", "filled", "commits"].forEach((field) => {
      const kept = [...section[field]];
      kept.forEach((_, position) => {
        section[field].splice(position, 1);
        try {
          if (allCasesMatch()) {
            surviving.push(`${section.name} ${field}[${position}]`);
          }
        } finally {
          section[field].splice(0, section[field].length, ...kept);
        }
      });
    });
    const index = SECTIONS.indexOf(section);
    SECTIONS.splice(index, 1);
    try {
      if (allCasesMatch()) {
        surviving.push(`section ${section.name}`);
      }
    } finally {
      SECTIONS.splice(index, 0, section);
    }
  });
  assert.deepEqual(surviving, []);
});
