import { isDeepStrictEqual } from "node:util";
import { pathToFileURL } from "node:url";
import {
  appConfigFile,
  caseFolders,
  desktop,
  readCases,
  runCase,
  runVitest,
  testFilesUnder,
  verdict,
} from "./vitest.mjs";

const failures = [];
const report = (status, text) => {
  process.stdout.write(`${status} ${text}\n`);
  if (status === "FAIL") failures.push(text);
};

const { config, cases } = readCases();
const appConfig = (await import(pathToFileURL(appConfigFile).href)).default;
report(
  isDeepStrictEqual(appConfig, config) ? "PASS" : "FAIL",
  "apps/desktop/vitest.config.mjs matches cases.json config exactly",
);

const folders = caseFolders();
report(
  isDeepStrictEqual(folders, Object.keys(cases).sort()) ? "PASS" : "FAIL",
  `case folders match cases.json: ${folders.length}`,
);

for (const [name, expected] of Object.entries(cases)) {
  const got = runCase(name);
  report(
    isDeepStrictEqual(got, [...expected].sort()) ? "PASS" : "FAIL",
    `${name}: expected [${expected.join(", ")}], got [${got.join(", ")}]`,
  );
}

const appFiles = testFilesUnder(desktop);
const app = verdict(runVitest(desktop), appFiles);
report(
  app.length === 0 ? "PASS" : "FAIL",
  `apps/desktop: ${appFiles.length} test file(s), problems [${app.join(", ")}]`,
);

process.stdout.write(`vitest-check: ${failures.length} failure(s)\n`);
process.exitCode = failures.length === 0 ? 0 : 1;
