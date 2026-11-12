import { readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const here = dirname(fileURLToPath(import.meta.url));
export const desktop = join(here, "..", "..", "apps", "desktop");
export const appConfigFile = join(desktop, "eslint.config.mjs");

const { ESLint } = createRequire(join(desktop, "package.json"))("eslint");

export const readCases = () =>
  JSON.parse(readFileSync(join(here, "cases.json"), "utf8"));

const walk = (folder) =>
  readdirSync(folder, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory()
      ? walk(join(folder, entry.name))
      : [join(folder, entry.name)],
  );

export const caseFilesOnDisk = () =>
  walk(join(here, "src"))
    .map((file) => relative(here, file).split(sep).join("/"))
    .sort();

export const violationLinter = (overrideConfig) =>
  new ESLint({
    cwd: here,
    overrideConfigFile: appConfigFile,
    cache: false,
    ...(overrideConfig === undefined ? {} : { overrideConfig }),
  });

export const lintCases = async (files, overrideConfig) => {
  const results = await violationLinter(overrideConfig).lintFiles(files);
  return new Map(
    results.map((result) => [
      relative(here, result.filePath).split(sep).join("/"),
      result.messages,
    ]),
  );
};
