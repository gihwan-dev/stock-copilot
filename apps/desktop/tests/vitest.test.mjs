import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import {
  appConfigFile,
  desktop,
  readCases,
  runCase,
  verdict,
  withConfig,
} from "../../../harness/vitest-check/vitest.mjs";

const appConfig = (await import(pathToFileURL(appConfigFile).href)).default;
const { cases } = readCases();

test("vitest is pinned to the version of architecture 5.2", () => {
  const manifest = JSON.parse(
    readFileSync(join(desktop, "package.json"), "utf8"),
  );
  assert.equal(manifest.devDependencies.vitest, "5.1.0");
});

test("removing any key of the Vitest test options makes some case fail", () => {
  const survivors = Object.keys(appConfig.test).filter((key) =>
    withConfig(
      {
        ...appConfig,
        test: Object.fromEntries(
          Object.entries(appConfig.test).filter(([other]) => other !== key),
        ),
      },
      (file) =>
        Object.entries(cases).every(
          ([name, expected]) =>
            JSON.stringify(runCase(name, file)) ===
            JSON.stringify([...expected].sort()),
        ),
    ),
  );
  assert.deepEqual(survivors, []);
});

const report = (statuses, extra = {}) => ({
  status: 0,
  report: {
    success: true,
    numFailedTests: 0,
    numFailedTestSuites: 0,
    numPendingTests: 0,
    numPendingTestSuites: 0,
    numTodoTests: 0,
    testResults: [{ assertionResults: statuses.map((status) => ({ status })) }],
    ...extra,
  },
});

test("the verdict reads skipped and todo tests from both the results and the totals", () => {
  assert.deepEqual(verdict(report(["passed"]), ["src/a.test.ts"]), []);
  assert.deepEqual(verdict(report(["passed", "skipped"]), ["x"]), ["skipped"]);
  assert.deepEqual(verdict(report(["passed", "pending"]), ["x"]), ["skipped"]);
  assert.deepEqual(verdict(report(["passed", "todo"]), ["x"]), ["todo"]);
  assert.deepEqual(
    verdict(report(["passed"], { numPendingTestSuites: 1 }), ["x"]),
    ["skipped"],
  );
  assert.deepEqual(verdict(report(["passed"], { numTodoTests: 1 }), ["x"]), [
    "todo",
  ]);
  assert.deepEqual(verdict(report([]), ["x"]), ["no-tests"]);
  assert.deepEqual(verdict(report([]), []), []);
  assert.deepEqual(verdict({ status: 1, report: report([]).report }, []), [
    "failed",
  ]);
  assert.deepEqual(verdict({ status: 0, report: undefined }, []), [
    "no-report",
  ]);
});
