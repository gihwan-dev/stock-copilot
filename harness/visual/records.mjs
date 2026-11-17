import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { isDeepStrictEqual } from "node:util";
import {
  existsSync,
  readdirSync,
  readFileSync,
  writeFileSync,
  mkdirSync,
  rmSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { settings, baselineProblems } from "./policy.mjs";
const expected = () =>
  settings.components.flatMap((component) =>
    settings.themes.map((theme) => ({
      component,
      theme,
      name: `${component}-${theme}-chromium-linux-arm64.png`,
    })),
  );
export const recordProblems = (record, images) => {
  if (
    record === null ||
    typeof record !== "object" ||
    !Array.isArray(record.measurements)
  )
    return ["실측 자료 누락 또는 형식 오류"];
  const problems = [];
  if (
    !isDeepStrictEqual(Object.keys(record).sort(), [
      "measurements",
      "settings",
    ]) ||
    !isDeepStrictEqual(record.settings, settings)
  )
    problems.push("실측 기준 환경 불일치");
  const entries = expected();
  if (
    !isDeepStrictEqual(
      Object.keys(images).sort(),
      entries.map((entry) => entry.name).sort(),
    )
  )
    problems.push("기준 PNG 목록 불일치");
  if (record.measurements.length !== entries.length)
    problems.push("실측 행 수 불일치");
  for (const { component, theme, name } of entries) {
    const rows = record.measurements.filter(
      (row) => row?.component === component && row?.theme === theme,
    );
    const bytes = images[name];
    if (rows.length !== 1 || bytes === undefined || bytes.length === 0) {
      problems.push(`실측 또는 PNG 누락: ${name}`);
      continue;
    }
    const hash = createHash("sha256").update(bytes).digest("hex");
    if (
      rows[0].bytes !== bytes.length ||
      rows[0].first !== hash ||
      rows[0].second !== hash
    )
      problems.push(`실측 해시/크기 불일치: ${name}`);
  }
  return problems;
};
export const storedProblems = (folder) => {
  if (!existsSync(join(folder, "measurement.json"))) return ["실측 자료 누락"];
  const record = (() => {
    try {
      return JSON.parse(readFileSync(join(folder, "measurement.json"), "utf8"));
    } catch {
      return undefined;
    }
  })();
  const names = readdirSync(folder);
  const images = Object.fromEntries(
    names
      .filter((name) => /\.png$/i.test(name))
      .map((name) => [name, readFileSync(join(folder, name))]),
  );
  const extras = names.filter(
    (name) => name !== "measurement.json" && !Object.hasOwn(images, name),
  );
  return [
    ...recordProblems(record, images),
    ...extras.map((name) => `기준 폴더의 알 수 없는 파일: ${name}`),
  ];
};
export const writeBaselineSet = async (folder, measurements, images) => {
  const record = { settings, measurements };
  const problems = recordProblems(record, images);
  if (problems.length) throw new Error(problems.join("\n"));
  const previous = existsSync(folder) ? readdirSync(folder) : [];
  const foreign = baselineProblems(previous);
  const unmanaged = previous.filter(
    (name) =>
      name !== "measurement.json" &&
      !Object.hasOwn(images, name) &&
      !name.endsWith("-chromium-linux-arm64.png"),
  );
  if (foreign.length || unmanaged.length)
    throw new Error([...foreign, ...unmanaged].join("\n"));
  const require = createRequire(
    join(resolve(import.meta.dirname, "../.."), "package.json"),
  );
  const { format } = require("prettier");
  const text = await format(JSON.stringify(record), { parser: "json" });
  mkdirSync(folder, { recursive: true });
  for (const [name, bytes] of Object.entries(images))
    writeFileSync(join(folder, name), bytes);
  for (const name of previous.filter(
    (name) =>
      name.endsWith("-chromium-linux-arm64.png") &&
      !Object.hasOwn(images, name),
  ))
    rmSync(join(folder, name));
  writeFileSync(join(folder, "measurement.json"), text);
};
