import { spawnSync } from "node:child_process";
import {
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const here = dirname(fileURLToPath(import.meta.url));
export const desktop = join(here, "..", "..", "apps", "desktop");
export const appConfigFile = join(desktop, "knip.json");
export const casesRoot = join(here, "cases");
const knipBin = join(desktop, "node_modules", ".bin", "knip");

export const readAppConfig = () =>
  JSON.parse(readFileSync(appConfigFile, "utf8"));

export const readCases = () =>
  JSON.parse(readFileSync(join(here, "cases.json"), "utf8"));

export const caseFolders = () =>
  readdirSync(casesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();

export const issueTypes = (report) =>
  [
    ...new Set(
      report.issues.flatMap((issue) =>
        Object.entries(issue)
          .filter(([, value]) => Array.isArray(value) && value.length > 0)
          .map(([type]) => type),
      ),
    ),
  ].sort();

export const issueItems = (report) =>
  report.issues
    .flatMap((issue) =>
      Object.entries(issue)
        .filter(([, value]) => Array.isArray(value))
        .flatMap(([type, values]) =>
          values.map((value) => `${type} ${issue.file} ${value.name}`),
        ),
    )
    .sort();

export const runKnip = (directory, configFile = appConfigFile, flags = []) => {
  const run = spawnSync(
    knipBin,
    [
      "--no-progress",
      "--config",
      configFile,
      "--directory",
      directory,
      "--reporter",
      "json",
      ...flags,
    ],
    { cwd: directory, encoding: "utf8" },
  );
  const parsed = /^\{/.test(run.stdout) ? JSON.parse(run.stdout) : undefined;
  return {
    status: run.status,
    types: parsed === undefined ? ["(no report)"] : issueTypes(parsed),
    items: parsed === undefined ? ["(no report)"] : issueItems(parsed),
    output: run.stdout + run.stderr,
  };
};

export const withConfig = (config, action) => {
  const scratch = mkdtempSync(join(tmpdir(), "knip-config-"));
  const file = join(scratch, "knip.json");
  writeFileSync(file, `${JSON.stringify(config)}\n`);
  const result = action(file);
  rmSync(scratch, { recursive: true, force: true });
  return result;
};

export const PRODUCTION_FLAGS = ["--strict"];

export const productionCasesRoot = join(here, "production-cases");

export const productionConfig = (config) => ({
  ...config,
  entry: config.entry.filter((pattern) => pattern.endsWith("!")),
});

export const readProductionAllowlist = () =>
  JSON.parse(readFileSync(join(here, "production-allowlist.json"), "utf8"));

export const productionFolders = () =>
  readdirSync(productionCasesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();

export const productionCaseVerdict = (
  name,
  configFile,
  flags = PRODUCTION_FLAGS,
) => {
  const run = runKnip(join(productionCasesRoot, name), configFile, flags);
  return { failed: run.status !== 0, types: run.types };
};

export const caseVerdict = (name, configFile) => {
  const run = runKnip(join(casesRoot, name), configFile);
  const failed = run.status !== 0;
  return { failed, types: run.types };
};

export const caseProblems = (expected, verdict) => [
  ...(verdict.failed === expected.length > 0
    ? []
    : [`exit ${verdict.failed ? "non-zero" : "zero"}`]),
  ...(JSON.stringify(verdict.types) === JSON.stringify([...expected].sort())
    ? []
    : [`types [${verdict.types.join(", ")}]`]),
];

export const APP_FLAGS = ["--treat-config-hints-as-errors"];
