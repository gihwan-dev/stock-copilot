import { readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { actualKeys } from "./verdict.mjs";

export const here = dirname(fileURLToPath(import.meta.url));
export const desktop = join(here, "..", "..", "apps", "desktop");
export const appConfigFile = join(desktop, "eslint.config.mjs");

const { ESLint } = createRequire(join(desktop, "package.json"))("eslint");
const { guardRuleBase } = await import(
  pathToFileURL(join(desktop, "eslint", "guard-plugin.mjs")).href
);
const { local } = await import(
  pathToFileURL(join(desktop, "eslint", "local-plugin.mjs")).href
);

export const localRuleMessages = Object.fromEntries(
  Object.entries(local.rules).map(([name, rule]) => [
    `local/${name}`,
    Object.values(rule.meta.messages),
  ]),
);

export const listRuleBase = (rule) =>
  rule.startsWith("guard/") ? guardRuleBase[rule.slice("guard/".length)] : rule;

const listBases = new Set([
  "no-restricted-syntax",
  "no-restricted-globals",
  "no-restricted-properties",
  "no-restricted-imports",
]);

const entryMessages = (base, options) =>
  base === "no-restricted-imports"
    ? options.flatMap((option) =>
        typeof option === "string"
          ? [option]
          : [...(option.paths ?? []), ...(option.patterns ?? [])].map(
              (entry) => (typeof entry === "string" ? entry : entry.message),
            ),
      )
    : options.map((entry) =>
        typeof entry === "string" ? entry : (entry.message ?? entry.name),
      );

const isOn = (entry) => {
  const severity = Array.isArray(entry) ? entry[0] : entry;
  return severity !== 0 && severity !== "off";
};

export const listEntriesOf = (config) =>
  Object.fromEntries([
    ...Object.entries(config.rules)
      .filter(([rule]) => listBases.has(listRuleBase(rule) ?? ""))
      .map(([rule, entry]) => [
        rule,
        entryMessages(
          listRuleBase(rule),
          Array.isArray(entry) ? entry.slice(1) : [],
        ),
      ]),
    ...Object.entries(config.rules)
      .filter(([rule, entry]) => rule in localRuleMessages && isOn(entry))
      .map(([rule]) => [rule, localRuleMessages[rule]]),
  ]);

export const mergeEntries = (maps) =>
  maps.reduce(
    (merged, map) =>
      Object.fromEntries(
        [...new Set([...Object.keys(merged), ...Object.keys(map)])].map(
          (rule) => [
            rule,
            [...new Set([...(merged[rule] ?? []), ...(map[rule] ?? [])])],
          ],
        ),
      ),
    {},
  );

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
  const linter = violationLinter(overrideConfig);
  const results = await linter.lintFiles(files);
  const entries = await Promise.all(
    results.map(async (result) => {
      const config = await linter.calculateConfigForFile(result.filePath);
      const listEntries = listEntriesOf(config);
      return [
        relative(here, result.filePath).split(sep).join("/"),
        {
          messages: result.messages,
          listEntries,
          keys: actualKeys(result.messages, listEntries),
        },
      ];
    }),
  );
  return new Map(entries);
};
