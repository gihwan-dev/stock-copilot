import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";

const ROOT = resolve(fileURLToPath(import.meta.url), "../../..");
export const TOKEN_FILE = "apps/desktop/design-tokens/tokens.json";
export const DERIVED_SPACING = Object.freeze([7, 9, 11]);
export const readInputs = () => ({
  source: JSON.parse(readFileSync(resolve(ROOT, "design/tokens.json"), "utf8")),
  tokens: JSON.parse(readFileSync(resolve(ROOT, TOKEN_FILE), "utf8")),
  architecture: readFileSync(resolve(ROOT, "docs/architecture.md"), "utf8"),
});

const themes = (value) =>
  typeof value === "object" && value !== null
    ? value
    : { light: value, dark: value };

const expectedTokens = (source) => {
  const expected = structuredClone(source);
  for (const section of Object.values(expected)) {
    if (!Array.isArray(section?.tokens)) continue;
    section.tokens = section.tokens.map((token) => ({
      ...token,
      value: themes(token.value),
    }));
  }
  for (const index of DERIVED_SPACING) {
    expected.spacing.tokens.push({
      name: `space-${index}`,
      value: { light: `${index * 4}px`, dark: `${index * 4}px` },
      usage: "design/README.md의 4px 기준 간격에서 유도",
    });
  }
  expected.spacing.tokens.sort(
    (left, right) => Number(left.name.slice(6)) - Number(right.name.slice(6)),
  );
  return expected;
};

const pixel = /^(?:0|[1-9]\d*)(?:\.\d+)?px$/u;
const integer = /^(?:0|[1-9]\d*)$/u;
const color = /^(?:#[\da-f]{6}|rgba\((?:\d{1,3}, ){3}(?:0(?:\.\d+)?|1)\))$/u;
const shadow =
  /^(?:0|\d+px)(?: (?:0|\d+px)){2,3} rgba\((?:\d{1,3}, ){3}(?:0(?:\.\d+)?|1)\)(?:, (?:0|\d+px)(?: (?:0|\d+px)){2,3} rgba\((?:\d{1,3}, ){3}(?:0(?:\.\d+)?|1)\))*$/u;
const formats = {
  color: (value) =>
    color.test(value) &&
    [...value.matchAll(/(\d+),/gu)].every((match) => Number(match[1]) <= 255),
  spacing: (value) => pixel.test(value) && Number(value.slice(0, -2)) % 4 === 0,
  radius: (value) => pixel.test(value),
  shadow: (value) => shadow.test(value),
  size: (value, name) =>
    name === "bubble-max-width"
      ? /^(?:[1-9]\d?|100)%$/u.test(value)
      : pixel.test(value),
  layout: (value, name) =>
    name === "grid-columns" ? integer.test(value) : pixel.test(value),
  zIndex: (value) => integer.test(value),
  duration: (value) => /^\d+ms$/u.test(value),
  easing: (value) =>
    /^cubic-bezier\((?:0(?:\.\d+)?|1), (?:0(?:\.\d+)?|1), (?:0(?:\.\d+)?|1), (?:0(?:\.\d+)?|1)\)$/u.test(
      value,
    ),
  opacity: (value) => /^(?:0(?:\.\d+)?|1)$/u.test(value),
  text: (value, name) =>
    name.startsWith("weight-")
      ? /^[1-9]00$/u.test(value)
      : name.startsWith("tracking-")
        ? /^-?\d+(?:\.\d+)?px$/u.test(value)
        : pixel.test(value),
};

const architectureNames = (architecture, source) => {
  const section =
    architecture.split("### 5.1 레이아웃 시스템")[1]?.split("### 5.2")[0] ?? "";
  const names = [...section.matchAll(/`([a-z][a-z0-9-]*\*?)`/gu)]
    .map((match) => match[1])
    .filter((name) =>
      /^(?:space-|bp-|content-|prose-|grid-columns$|chat-width|sidebar-|dialog-width|toolbar-height$|chart-height|row-height|control-height|window-min-|gap-hairline$)/u.test(
        name,
      ),
    );
  const ranges = [...section.matchAll(/`space-(\d+)`~`space-(\d+)`/gu)].flatMap(
    (match) =>
      Array.from(
        { length: Number(match[2]) - Number(match[1]) + 1 },
        (_, index) => `space-${Number(match[1]) + index}`,
      ),
  );
  return [
    ...new Set([
      ...names.flatMap((name) =>
        name.endsWith("*")
          ? source.size.tokens
              .filter((token) => token.name.startsWith(name.slice(0, -1)))
              .map((token) => token.name)
          : [name],
      ),
      ...ranges,
    ]),
  ];
};

export const tokenProblems = ({ source, tokens, architecture }) => {
  const problems = [];
  if (!isDeepStrictEqual(tokens, expectedTokens(source)))
    problems.push("원본 또는 문서에서 유도한 토큰과 다르다");
  const names = new Set();
  for (const [group, section] of Object.entries(tokens)) {
    if (!Array.isArray(section?.tokens)) continue;
    const byName = new Map(section.tokens.map((token) => [token.name, token]));
    for (const token of section.tokens) {
      if (
        !/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(token.name) ||
        names.has(token.name)
      )
        problems.push(`잘못되거나 중복된 이름: ${token.name}`);
      names.add(token.name);
      if (
        !isDeepStrictEqual(Object.keys(token.value ?? {}).sort(), [
          "dark",
          "light",
        ])
      ) {
        problems.push(`테마 키 불일치: ${token.name}`);
        continue;
      }
      for (const theme of ["light", "dark"]) {
        let value = token.value[theme];
        const seen = new Set([token.name]);
        while (typeof value === "string" && /^\{[a-z0-9-]+\}$/u.test(value)) {
          const reference = value.slice(1, -1);
          if (
            group !== "color" ||
            seen.has(reference) ||
            !byName.has(reference)
          ) {
            problems.push(`참조 오류: ${token.name}/${theme}`);
            value = undefined;
            break;
          }
          seen.add(reference);
          value = byName.get(reference).value?.[theme];
        }
        if (typeof value !== "string" || !formats[group]?.(value, token.name))
          problems.push(`값 형식 오류: ${token.name}/${theme}`);
      }
    }
  }
  for (const name of architectureNames(architecture, source)) {
    if (!names.has(name))
      problems.push(`architecture 5.1 필수 토큰 누락: ${name}`);
  }
  for (const group of tokens.type.groups) {
    if (!(group.family in tokens.type.families))
      problems.push(`글꼴 참조 오류: ${group.family}`);
    for (const style of group.styles) {
      if (
        !pixel.test(style.fontSize) ||
        !pixel.test(style.lineHeight) ||
        !/^[1-9]00$/u.test(String(style.fontWeight)) ||
        (style.letterSpacing !== undefined &&
          !/^-?\d+(?:\.\d+)?px$/u.test(style.letterSpacing))
      )
        problems.push(`글자 형식 오류: ${style.name}`);
    }
  }
  for (const font of tokens.type.fonts) {
    if (
      !/^fonts\/[A-Za-z0-9-]+\.woff2$/u.test(font.file) ||
      !/^[1-9]00$/u.test(font.weight) ||
      font.style !== "normal"
    )
      problems.push(`글꼴 형식 오류: ${font.file}`);
  }
  return problems;
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const problems = tokenProblems(readInputs());
    problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
    process.stdout.write(`ds-tokens: ${problems.length} failure(s)\n`);
    process.exitCode = problems.length === 0 ? 0 : 1;
  } catch (error) {
    process.stderr.write(`ds-tokens: ${error.message}\n`);
    process.exitCode = 1;
  }
}
