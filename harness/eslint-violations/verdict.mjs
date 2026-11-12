import { isDeepStrictEqual } from "node:util";

const directiveLabels = [
  [/^Unused eslint-disable directive/, "Unused eslint-disable directive"],
  [/^Unused inline config/, "Unused inline config"],
  [/has no effect because you have 'noInlineConfig'/, "noInlineConfig"],
];

const severityPrefix = (message) => (message.severity === 2 ? "" : "warning ");

const entryOf = (message, entries) =>
  entries
    .filter((entry) => message.message.includes(entry))
    .sort((a, b) => b.length - a.length)[0];

export const messageKey = (message, listEntries) => {
  if (message.ruleId === null) {
    const label = directiveLabels.find(([pattern]) =>
      pattern.test(message.message),
    );
    return `${severityPrefix(message)}(null) ${label === undefined ? message.message : label[1]}`;
  }
  if (message.ruleId.startsWith("boundaries/"))
    return `${severityPrefix(message)}${message.ruleId} :: ${message.message}`;
  const entries = listEntries[message.ruleId];
  const entry = entries === undefined ? undefined : entryOf(message, entries);
  return entries === undefined
    ? `${severityPrefix(message)}${message.ruleId}`
    : `${severityPrefix(message)}${message.ruleId} :: ${entry ?? `(no entry) ${message.message}`}`;
};

export const actualKeys = (messages, listEntries) =>
  [
    ...new Set(messages.map((message) => messageKey(message, listEntries))),
  ].sort();

export const caseProblems = (expect, keys) => {
  if (expect === "KNOWN_GAP") return [];
  const want = [...expect].sort();
  return isDeepStrictEqual(keys, want)
    ? []
    : [
        ...want
          .filter((key) => !keys.includes(key))
          .map((key) => `missing ${key}`),
        ...keys
          .filter((key) => !want.includes(key))
          .map((key) => `unexpected ${key}`),
      ];
};

export const knownGapProblems = (spec, keys) =>
  Array.isArray(spec.reports)
    ? caseProblems(spec.reports, keys).map(
        (problem) => `known gap changed: ${problem}`,
      )
    : ["known gap without the exact reports it gets today"];

const expectedKeysOf = (cases, row) =>
  Object.values(cases)
    .filter((spec) => row === undefined || spec.row === row)
    .flatMap((spec) => (Array.isArray(spec.expect) ? spec.expect : []));

export const coverageProblems = (rows, cases) => [
  ...Object.entries(cases)
    .filter(([, spec]) => !(spec.row in rows))
    .map(([file, spec]) => `${file}: row "${spec.row}" is not in rows`),
  ...Object.entries(rows).flatMap(([row, keys]) =>
    keys
      .filter(
        (key) =>
          !expectedKeysOf(cases, row).some(
            (expected) => expected === key || expected.startsWith(`${key} :: `),
          ),
      )
      .map((key) => `row "${row}": no case expects ${key}`),
  ),
];

export const entryCoverageProblems = (listEntries, cases) => {
  const expected = new Set(expectedKeysOf(cases, undefined));
  return Object.entries(listEntries).flatMap(([rule, entries]) =>
    entries
      .map((entry) => `${rule} :: ${entry}`)
      .filter((key) => !expected.has(key))
      .map((key) => `no case expects ${key}`),
  );
};

export const listingProblems = (onDisk, listed) => {
  const disk = new Set(onDisk);
  const list = new Set(listed);
  return [
    ...onDisk
      .filter((file) => !list.has(file))
      .map((file) => `${file} is not in cases.json`),
    ...listed
      .filter((file) => !disk.has(file))
      .map((file) => `${file} is listed but missing`),
  ];
};
