import { readFileSync } from "node:fs";
import { join } from "node:path";
import { isDeepStrictEqual } from "node:util";
import { caseFilesOnDisk, here, lintCases, readCases } from "./lint.mjs";
import {
  caseProblems,
  coverageProblems,
  describeMessages,
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
    { extends: "../../apps/desktop/tsconfig.json", include: ["src"] },
  )
    ? "PASS"
    : "FAIL",
  "harness/eslint-violations/tsconfig.json only extends the app config",
);

const onDisk = caseFilesOnDisk();
reportAll(
  listingProblems(onDisk, Object.keys(cases)),
  `case files match cases.json: ${onDisk.length}`,
);
reportAll(coverageProblems(rows, cases), "every documented rule has a case");

const linted = await lintCases(Object.keys(cases));
for (const [file, spec] of Object.entries(cases)) {
  const messages = linted.get(file) ?? [];
  const problems = caseProblems(spec.expect, messages);
  if (spec.expect === "KNOWN_GAP") {
    report(
      messages.length === 0 ? "GAP " : "GAP?",
      `${file}: known gap, got ${describeMessages(messages)}`,
    );
  } else if (problems.length === 0) {
    report("PASS", `${file}`);
  } else {
    problems.forEach((problem) => report("FAIL", `${file}: ${problem}`));
  }
}

process.stdout.write(`eslint-violations: ${failures.length} failure(s)\n`);
process.exitCode = failures.length === 0 ? 0 : 1;
