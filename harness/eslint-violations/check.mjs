import { readFileSync } from "node:fs";
import { join } from "node:path";
import { isDeepStrictEqual } from "node:util";
import {
  caseFilesOnDisk,
  here,
  lintCases,
  mergeEntries,
  readCases,
} from "./lint.mjs";
import {
  caseProblems,
  coverageProblems,
  entryCoverageProblems,
  listingProblems,
} from "./verdict.mjs";

const failures = [];
const report = (status, text) => {
  process.stdout.write(`${status} ${text}\n`);
  if (status === "FAIL") failures.push(text);
};
const reportAll = (problems, passText) =>
  problems.length === 0
    ? report("PASS", passText)
    : problems.forEach((problem) => report("FAIL", problem));

const { rows, cases } = readCases();

report(
  isDeepStrictEqual(
    JSON.parse(readFileSync(join(here, "tsconfig.json"), "utf8")),
    {
      extends: "../../apps/desktop/tsconfig.json",
      compilerOptions: {
        rootDir: "src",
        paths: {
          "*": [
            "../../apps/desktop/node_modules/@types/*",
            "../../apps/desktop/node_modules/*",
          ],
        },
      },
      include: ["src"],
    },
  )
    ? "PASS"
    : "FAIL",
  "harness/eslint-violations/tsconfig.json extends the app config and only points module lookup at the app",
);

const onDisk = caseFilesOnDisk();
reportAll(
  listingProblems(onDisk, Object.keys(cases)),
  `case files match cases.json: ${onDisk.length}`,
);
reportAll(coverageProblems(rows, cases), "every documented rule has a case");

const linted = await lintCases(Object.keys(cases));
reportAll(
  entryCoverageProblems(
    mergeEntries([...linted.values()].map((result) => result.listEntries)),
    cases,
  ),
  "every entry of every restricted list has a case",
);
for (const [file, spec] of Object.entries(cases)) {
  const keys = linted.get(file)?.keys ?? [];
  const problems = caseProblems(spec.expect, keys);
  if (spec.expect === "KNOWN_GAP") {
    report(
      keys.length === 0 ? "GAP " : "GAP?",
      `${file}: known gap, got [${keys.join(", ")}]`,
    );
  } else if (problems.length === 0) {
    report("PASS", file);
  } else {
    problems.forEach((problem) => report("FAIL", `${file}: ${problem}`));
  }
}

process.stdout.write(`eslint-violations: ${failures.length} failure(s)\n`);
process.exitCode = failures.length === 0 ? 0 : 1;
