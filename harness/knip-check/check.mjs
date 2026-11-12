import { isDeepStrictEqual } from "node:util";
import {
  APP_FLAGS,
  appConfigFile,
  caseFolders,
  caseProblems,
  caseVerdict,
  desktop,
  readAppConfig,
  PRODUCTION_FLAGS,
  productionCaseVerdict,
  productionConfig,
  productionFolders,
  readCases,
  readProductionAllowlist,
  runKnip,
  withConfig,
} from "./knip.mjs";

const failures = [];
const report = (status, text) => {
  process.stdout.write(`${status} ${text}\n`);
  if (status === "FAIL") failures.push(text);
};

const { config, withoutCase, addedWithDependencies, cases, production } =
  readCases();
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

const productionNames = productionFolders();
report(
  isDeepStrictEqual(productionNames, Object.keys(production).sort())
    ? "PASS"
    : "FAIL",
  `production case folders match cases.json: ${productionNames.length}`,
);
withConfig(productionConfig(appConfig), (file) => {
  for (const [name, expected] of Object.entries(production)) {
    const problems = caseProblems(expected, productionCaseVerdict(name, file));
    report(
      problems.length === 0 ? "PASS" : "FAIL",
      `production ${name}: expected [${expected.join(", ")}]${problems.length === 0 ? "" : `, got ${problems.join(", ")}`}`,
    );
  }
  const allowed = readProductionAllowlist().entries.map((entry) => entry.issue);
  const found = runKnip(desktop, file, PRODUCTION_FLAGS).items;
  const unexpected = found.filter((item) => !allowed.includes(item));
  const stale = allowed.filter((item) => !found.includes(item));
  unexpected.forEach((item) =>
    report(
      "FAIL",
      `apps/desktop production: ${item} is not in production-allowlist.json`,
    ),
  );
  stale.forEach((item) =>
    report(
      "FAIL",
      `apps/desktop production: ${item} is allowlisted but no longer reported`,
    ),
  );
  if (unexpected.length === 0 && stale.length === 0)
    report(
      "PASS",
      `apps/desktop production: exactly the ${allowed.length} allowlisted issue(s)`,
    );
});

process.stdout.write(`knip-check: ${failures.length} failure(s)\n`);
process.exitCode = failures.length === 0 ? 0 : 1;
