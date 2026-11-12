import { isDeepStrictEqual } from "node:util";
import {
  APP_FLAGS,
  appConfigFile,
  caseFolders,
  caseProblems,
  caseVerdict,
  desktop,
  readAppConfig,
  readCases,
  runKnip,
} from "./knip.mjs";

const failures = [];
const report = (status, text) => {
  process.stdout.write(`${status} ${text}\n`);
  if (status === "FAIL") failures.push(text);
};

const { config, withoutCase, addedWithDependencies, cases } = readCases();
const appConfig = readAppConfig();
report(
  isDeepStrictEqual(appConfig, config) ? "PASS" : "FAIL",
  "apps/desktop/knip.json matches cases.json config exactly",
);

const folders = caseFolders();
report(
  isDeepStrictEqual(folders, Object.keys(cases).sort()) ? "PASS" : "FAIL",
  `case folders match cases.json: ${folders.length}`,
);

const expectedTypes = new Set([...withoutCase, ...Object.values(cases).flat()]);
const checkedTypes = [...appConfig.include, ...addedWithDependencies];
const uncovered = checkedTypes.filter((type) => !expectedTypes.has(type));
report(
  uncovered.length === 0 ? "PASS" : "FAIL",
  `every included issue type has a case or is listed without one: missing [${uncovered.join(", ")}]`,
);
const notError = Object.entries(appConfig.rules)
  .filter(([, level]) => level !== "error")
  .map(([type]) => type);
report(
  notError.length === 0 &&
    isDeepStrictEqual(
      Object.keys(appConfig.rules).sort(),
      [...checkedTypes].sort(),
    ) &&
    appConfig.include.includes("dependencies")
    ? "PASS"
    : "FAIL",
  `every included issue type is an error: not error [${notError.join(", ")}]`,
);

for (const [name, expected] of Object.entries(cases)) {
  const verdict = caseVerdict(name, appConfigFile);
  const problems = caseProblems(expected, verdict);
  report(
    problems.length === 0 ? "PASS" : "FAIL",
    `${name}: expected [${expected.join(", ")}]${problems.length === 0 ? "" : `, got ${problems.join(", ")}`}`,
  );
}

const app = runKnip(desktop, appConfigFile, APP_FLAGS);
report(
  app.status === 0 && app.types.length === 0 ? "PASS" : "FAIL",
  `apps/desktop: knip reports [${app.types.join(", ")}]`,
);
if (app.status !== 0) process.stdout.write(app.output);

process.stdout.write(`knip-check: ${failures.length} failure(s)\n`);
process.exitCode = failures.length === 0 ? 0 : 1;
