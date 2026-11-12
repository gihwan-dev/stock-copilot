import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const here = dirname(fileURLToPath(import.meta.url));
export const desktop = join(here, "..", "..", "apps", "desktop");
export const appConfigFile = join(desktop, "vitest.config.mjs");
const vitestBin = join(desktop, "node_modules", ".bin", "vitest");

const walk = (folder) =>
  existsSync(folder)
    ? readdirSync(folder, { withFileTypes: true }).flatMap((entry) =>
        entry.isDirectory()
          ? walk(join(folder, entry.name))
          : [join(folder, entry.name)],
      )
    : [];

export const testFilesUnder = (root) =>
  walk(join(root, "src"))
    .map((file) => relative(root, file).split(sep).join("/"))
    .filter((file) => /\.test\.tsx?$/.test(file))
    .sort();

const skippedStatuses = new Set(["skipped", "pending"]);

export const verdict = ({ status, report }, testFiles) => {
  if (report === undefined) return ["no-report"];
  const statuses = report.testResults.flatMap((file) =>
    file.assertionResults.map((result) => result.status),
  );
  const count = (predicate) => statuses.filter(predicate).length;
  const skipped =
    count((s) => skippedStatuses.has(s)) +
    report.numPendingTests +
    report.numPendingTestSuites;
  const todo = count((s) => s === "todo") + report.numTodoTests;
  const failed =
    status !== 0 ||
    !report.success ||
    report.numFailedTests > 0 ||
    report.numFailedTestSuites > 0 ||
    count((s) => s === "failed") > 0;
  const passedIn = (file) =>
    (
      report.testResults.find((result) => result.name.endsWith(`/${file}`))
        ?.assertionResults ?? []
    ).filter((result) => result.status === "passed").length;
  const emptyFiles = testFiles.filter((file) => passedIn(file) === 0);
  return [
    ...(failed ? ["failed"] : []),
    ...(skipped > 0 ? ["skipped"] : []),
    ...(todo > 0 ? ["todo"] : []),
    ...(emptyFiles.length > 0 ? ["no-tests"] : []),
  ];
};

export const runVitest = (root, configFile = appConfigFile) => {
  const scratch = mkdtempSync(join(tmpdir(), "vitest-check-"));
  const output = join(scratch, "report.json");
  const run = spawnSync(
    vitestBin,
    [
      "run",
      "--config",
      configFile,
      "--root",
      root,
      "--reporter=json",
      `--outputFile.json=${output}`,
    ],
    { cwd: root, encoding: "utf8" },
  );
  const report = existsSync(output)
    ? JSON.parse(readFileSync(output, "utf8"))
    : undefined;
  rmSync(scratch, { recursive: true, force: true });
  return { status: run.status, report, output: run.stdout + run.stderr };
};

export const withConfig = (config, action) => {
  const scratch = mkdtempSync(join(tmpdir(), "vitest-config-"));
  const file = join(scratch, "vitest.config.mjs");
  writeFileSync(file, `export default ${JSON.stringify(config)};\n`);
  const result = action(file);
  rmSync(scratch, { recursive: true, force: true });
  return result;
};

export const casesRoot = join(here, "cases");

export const readCases = () =>
  JSON.parse(readFileSync(join(here, "cases.json"), "utf8"));

export const caseFolders = () =>
  readdirSync(casesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();

export const runCase = (name, configFile) => {
  const root = join(casesRoot, name);
  return verdict(runVitest(root, configFile), testFilesUnder(root)).sort();
};
