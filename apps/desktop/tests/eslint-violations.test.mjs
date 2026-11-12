import { strict as assert } from "node:assert";
import { test } from "node:test";
import { bans, importBans } from "../eslint/bans.mjs";
import {
  lintCases,
  mergeEntries,
  readCases,
} from "../../../harness/eslint-violations/lint.mjs";
import {
  caseProblems,
  coverageProblems,
  entryCoverageProblems,
} from "../../../harness/eslint-violations/verdict.mjs";

const { rows, cases } = readCases();
const allFiles = Object.keys(cases);

const ruleOf = (key) => key.replace(/^warning /, "").split(" :: ")[0];

const expectations = Object.entries(cases).flatMap(([file, spec]) =>
  Array.isArray(spec.expect) ? spec.expect.map((key) => ({ file, key })) : [],
);

const ruleIds = [
  ...new Set(
    expectations
      .map(({ key }) => ruleOf(key))
      .filter((rule) => !rule.startsWith("(null)"))
      .filter((rule) => /^[^\s]+$/.test(rule))
      .filter((rule) => !/^(local|boundaries)\//.test(rule)),
  ),
].sort();

const filesWhere = (predicate) => [
  ...new Set(
    expectations.filter(({ key }) => predicate(key)).map(({ file }) => file),
  ),
];

const failingAfter = async (files, overrideConfig) => {
  const linted = await lintCases(files, overrideConfig);
  return files.filter(
    (file) =>
      caseProblems(cases[file].expect, linted.get(file)?.keys ?? []).length > 0,
  );
};

const survivorsOf = async (variants) =>
  (
    await Promise.all(
      variants.map(async ({ label, files, overrideConfig }) => {
        const failing = await failingAfter(files, overrideConfig);
        return { label, survived: files.filter((f) => !failing.includes(f)) };
      }),
    )
  ).filter(({ survived }) => survived.length > 0);

test("the documented rule list of every harness 4.2 row has a case", () => {
  assert.deepEqual(coverageProblems(rows, cases), []);
});

test("every case reports exactly its expected set with the app configuration", async () => {
  assert.deepEqual(await failingAfter(allFiles, undefined), []);
});

test("every entry of every restricted list in the effective configuration has a case", async () => {
  const linted = await lintCases(allFiles, undefined);
  const entries = mergeEntries(
    [...linted.values()].map((result) => result.listEntries),
  );
  assert.deepEqual(entryCoverageProblems(entries, cases), []);
  assert.notDeepEqual(
    entryCoverageProblems(
      { ...entries, "guard/no-network": ["새 항목 금지"] },
      cases,
    ),
    [],
  );
});

test("an extra or missing problem makes a case fail", () => {
  const [file, spec] = Object.entries(cases).find(
    ([, candidate]) =>
      Array.isArray(candidate.expect) && candidate.expect.length > 0,
  );
  assert.notDeepEqual(
    caseProblems(spec.expect, [...spec.expect, "no-console"].sort()),
    [],
    file,
  );
  assert.notDeepEqual(caseProblems(spec.expect, spec.expect.slice(1)), []);
  assert.notDeepEqual(caseProblems([], ["no-console"]), []);
});

test("lowering any expected rule to a warning makes its cases fail", async () => {
  assert.deepEqual(
    await survivorsOf(
      ruleIds.map((rule) => ({
        label: rule,
        files: filesWhere((key) => ruleOf(key) === rule),
        overrideConfig: { files: ["src/**"], rules: { [rule]: "warn" } },
      })),
    ),
    [],
  );
});

test("turning off any expected rule makes its cases fail", async () => {
  assert.deepEqual(
    await survivorsOf(
      ruleIds.map((rule) => ({
        label: rule,
        files: filesWhere((key) => ruleOf(key) === rule),
        overrideConfig: { files: ["src/**"], rules: { [rule]: "off" } },
      })),
    ),
    [],
  );
});

test("removing any single entry from a restricted list makes its cases fail", async () => {
  const listVariants = Object.entries(bans).flatMap(([rule, entries]) =>
    entries.map((entry) => ({
      label: `${rule} :: ${entry.message}`,
      files: filesWhere((key) => key === `${rule} :: ${entry.message}`),
      overrideConfig: {
        files: ["src/**"],
        rules: {
          [rule]:
            entries.length === 1
              ? "off"
              : ["error", ...entries.filter((other) => other !== entry)],
        },
      },
    })),
  );
  const importVariants = Object.entries(importBans).flatMap(([rule, options]) =>
    options.patterns.map((pattern) => ({
      label: `${rule} :: ${pattern.message}`,
      files: filesWhere((key) => key === `${rule} :: ${pattern.message}`),
      overrideConfig: {
        files: ["src/**"],
        rules: {
          [rule]: [
            "error",
            {
              patterns: options.patterns.filter((other) => other !== pattern),
            },
          ],
        },
      },
    })),
  );
  const variants = [...listVariants, ...importVariants];
  assert.deepEqual(
    variants.filter(({ files }) => files.length === 0).map((v) => v.label),
    [],
  );
  assert.deepEqual(await survivorsOf(variants), []);
});

test("unused disable directives and inline configs fail only while reported as errors", async () => {
  const files = filesWhere((key) => key.startsWith("(null) Unused"));
  assert.notDeepEqual(files, []);
  const failing = await failingAfter(files, {
    linterOptions: {
      reportUnusedDisableDirectives: "warn",
      reportUnusedInlineConfigs: "warn",
    },
  });
  assert.deepEqual(failing.sort(), [...files].sort());
});
