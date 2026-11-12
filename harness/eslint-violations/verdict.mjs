import { isDeepStrictEqual } from "node:util";

const matches = (want, message) =>
  message.ruleId === want.rule &&
  (want.message === undefined || message.message.includes(want.message));

const label = (want) =>
  want.message === undefined
    ? `${want.rule}`
    : `${want.rule} ("${want.message}")`;

export const describeMessages = (messages) =>
  messages.length === 0
    ? "nothing"
    : [
        ...new Set(
          messages.map(
            (message) =>
              `${message.severity === 2 ? "error" : "warning"} ${message.ruleId ?? `(no rule) ${message.message}`}`,
          ),
        ),
      ]
        .sort()
        .join(", ");

export const caseProblems = (expect, messages) => {
  if (expect === "KNOWN_GAP") return [];
  if (expect === "clean")
    return messages.length === 0
      ? []
      : [`expected no problems, got ${describeMessages(messages)}`];
  const missing = expect.errors
    .filter(
      (want) =>
        !messages.some(
          (message) => message.severity === 2 && matches(want, message),
        ),
    )
    .map((want) => `missing error ${label(want)}`);
  const present = (expect.absent ?? [])
    .filter((rule) => messages.some((message) => message.ruleId === rule))
    .map((rule) => `unexpected ${rule}`);
  return [...missing, ...present];
};

export const coverageProblems = (rows, cases) => {
  const unknownRows = Object.entries(cases)
    .filter(([, spec]) => !(spec.row in rows))
    .map(([file, spec]) => `${file}: row "${spec.row}" is not in rows`);
  const uncovered = Object.entries(rows).flatMap(([row, wants]) =>
    wants
      .filter(
        (want) =>
          !Object.values(cases).some(
            (spec) =>
              spec.row === row &&
              typeof spec.expect === "object" &&
              spec.expect.errors.some((error) =>
                isDeepStrictEqual(error, want),
              ),
          ),
      )
      .map((want) => `row "${row}": no case expects ${label(want)}`),
  );
  return [...unknownRows, ...uncovered];
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
