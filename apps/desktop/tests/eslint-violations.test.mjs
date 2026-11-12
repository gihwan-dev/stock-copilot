import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  lintCases,
  readCases,
} from "../../../harness/eslint-violations/lint.mjs";
import {
  caseProblems,
  coverageProblems,
} from "../../../harness/eslint-violations/verdict.mjs";

const { rows, cases } = readCases();

const expectations = Object.entries(cases).flatMap(([file, spec]) =>
  typeof spec.expect === "object"
    ? spec.expect.errors.map((want) => ({ file, want }))
    : [],
);

const ruleIds = [
  ...new Set(
    expectations.map(({ want }) => want.rule).filter((rule) => rule !== null),
  ),
].sort();

const filesExpecting = (rule) => [
  ...new Set(
    expectations
      .filter(({ want }) => want.rule === rule)
      .map(({ file }) => file),
  ),
];

const failingAfter = async (files, overrideConfig) => {
  const linted = await lintCases(files, overrideConfig);
  return files.filter(
    (file) =>
      caseProblems(cases[file].expect, linted.get(file) ?? []).length > 0,
  );
};

test("the documented rule list of every harness 4.2 row has a case", () => {
  assert.deepEqual(coverageProblems(rows, cases), []);
});

test("every case passes with the app configuration", async () => {
  const files = Object.keys(cases);
  assert.deepEqual(await failingAfter(files, undefined), []);
});

test("lowering any expected rule to a warning makes its cases fail", async () => {
  const survivors = await Promise.all(
    ruleIds.map(async (rule) => {
      const files = filesExpecting(rule);
      const failing = await failingAfter(files, {
        files: ["src/**"],
        rules: { [rule]: "warn" },
      });
      return { rule, survived: files.filter((f) => !failing.includes(f)) };
    }),
  );
  assert.deepEqual(
    survivors.filter(({ survived }) => survived.length > 0),
    [],
  );
});

test("turning off any expected rule makes its cases fail", async () => {
  const survivors = await Promise.all(
    ruleIds.map(async (rule) => {
      const files = filesExpecting(rule);
      const failing = await failingAfter(files, {
        files: ["src/**"],
        rules: { [rule]: "off" },
      });
      return { rule, survived: files.filter((f) => !failing.includes(f)) };
    }),
  );
  assert.deepEqual(
    survivors.filter(({ survived }) => survived.length > 0),
    [],
  );
});

test("removing one restricted selector or entry makes its cases fail", async () => {
  const files = Object.keys(cases);
  const linted = await lintCases(files, undefined);
  const survivors = expectations
    .filter(({ want }) => want.message !== undefined)
    .filter(({ file, want }) => {
      const without = (linted.get(file) ?? []).filter(
        (message) =>
          !(
            message.ruleId === want.rule &&
            message.message.includes(want.message)
          ),
      );
      return caseProblems(cases[file].expect, without).length === 0;
    });
  assert.deepEqual(survivors, []);
});

test("unused disable directives and inline configs fail only while reported as errors", async () => {
  const files = filesExpecting(null);
  assert.notDeepEqual(files, []);
  const failing = await failingAfter(files, {
    linterOptions: {
      reportUnusedDisableDirectives: "warn",
      reportUnusedInlineConfigs: "warn",
    },
  });
  assert.deepEqual(failing.sort(), [...files].sort());
});
