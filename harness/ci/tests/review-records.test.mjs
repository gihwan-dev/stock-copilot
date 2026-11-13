import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  DONE_STATES,
  PROGRESS_FILE,
  REVIEWS_DIRECTORY,
  SECTIONS,
  TABLE_HEADER,
  reviewProblems,
} from "../review-records/check.mjs";
import { repoRoot, scratch } from "./signature-repo.mjs";

const CASES_FILE = "harness/ci/review-records/cases.json";

const cases = JSON.parse(
  readFileSync(join(repoRoot, CASES_FILE), "utf8"),
).cases;

const materialize = (entry) => {
  const directory = scratch("review-records-");
  mkdirSync(join(directory, REVIEWS_DIRECTORY), { recursive: true });
  writeFileSync(join(directory, PROGRESS_FILE), entry.progress);
  Object.entries(entry.reviews).forEach(([name, text]) =>
    writeFileSync(join(directory, REVIEWS_DIRECTORY, name), text),
  );
  return directory;
};

const problemsOf = (entry) => {
  const directory = materialize(entry);
  try {
    return reviewProblems(directory);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
};

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
  const directory = materialize(entry);
  try {
    const finished = spawnSync(
      "node",
      ["harness/ci/review-records/check.mjs", directory],
      { cwd: repoRoot, encoding: "utf8" },
    );
    assert.equal(finished.status, 1);
    assert.match(finished.stdout, /FAIL docs\/progress\.md:5 H-02/u);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("the check keys on the reviewed table header, done states and sections", () => {
  assert.deepEqual(TABLE_HEADER, ["작업", "상태", "대상 커밋", "리뷰 기록"]);
  assert.deepEqual(DONE_STATES, ["완료", "일부 완료"]);
  assert.deepEqual(
    SECTIONS.map((section) => section.name),
    ["커밋 표", "리뷰 표", "측정 표"],
  );
});

test("dropping any required table column or section makes some case pass that should fail", () => {
  const original = SECTIONS.map((section) => ({
    ...section,
    table: [...section.table],
  }));
  const surviving = [];
  try {
    SECTIONS.forEach((section, index) => {
      const saved = SECTIONS[index];
      SECTIONS.splice(index, 1);
      if (
        cases.every(
          (entry) =>
            JSON.stringify(problemsOf(entry)) ===
            JSON.stringify(entry.problems),
        )
      ) {
        surviving.push(`section ${section.name}`);
      }
      SECTIONS.splice(index, 0, saved);
      section.table.forEach((column, position) => {
        const kept = [...section.table];
        section.table.splice(position, 1);
        if (
          cases.every(
            (entry) =>
              JSON.stringify(problemsOf(entry)) ===
              JSON.stringify(entry.problems),
          )
        ) {
          surviving.push(`${section.name} column ${column}`);
        }
        section.table.splice(0, section.table.length, ...kept);
      });
    });
  } finally {
    SECTIONS.splice(0, SECTIONS.length, ...original);
  }
  assert.deepEqual(surviving, []);
});
