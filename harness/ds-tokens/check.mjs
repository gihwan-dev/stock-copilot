import { readFileSync, existsSync } from "node:fs";
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

const ARCHITECTURE_EXCLUSIONS = Object.freeze([
  "features",
  "AppShell",
  "platform-macos",
  "narrow",
  "medium",
  "wide",
  "ScrollArea",
  "shared/ds",
  "scrollbar-width: none",
  "html",
  "body",
  "Dialog",
  "Popover",
  "Menu",
  "Table",
  "ChatInput",
  "overflow: hidden",
  "clip",
  "Text",
  "word-break: keep-all",
  "overflow-wrap: anywhere",
  "Heading",
  "overflow: 'truncate'",
  "Money",
  "Percent",
  "Quantity",
  "Probability",
  "PriceChange",
  "Stack",
  "Inline",
  "Split",
  "GridItem",
  "min-width: 0",
  "min-height: 0",
  "position: absolute",
  "fixed",
  "sticky",
  "Tooltip",
  "Toast",
  "vh",
  "vw",
  "dvh",
  "app",
  "Page",
  "Section",
  "Grid",
  "Card",
  "Toolbar",
  "Divider",
  "design/tokens.json",
]);
const Z_ORDER = Object.freeze([
  "z-sticky",
  "z-panel",
  "z-popover",
  "z-modal",
  "z-toast",
]);
const WILDCARDS = Object.freeze({
  "z-*": Z_ORDER,
  "chart-height-*": Object.freeze([
    "chart-height-sm",
    "chart-height",
    "chart-height-lg",
  ]),
});
const ARCHITECTURE_NAME_COUNT = 29;

const architectureNames = (architecture) => {
  const problems = [];
  const match =
    /^### 5\.1 레이아웃 시스템\n([\s\S]*?)^### 5\.2 확정한 라이브러리(?:\r?\n|$)/mu.exec(
      architecture,
    );
  if (match === null)
    return { names: [], problems: ["architecture 5.1 구간 표지 오류"] };
  const section = match[1];
  const names = [
    ...new Set(
      [...section.matchAll(/`([^`\n]+)`/gu)]
        .map((entry) => entry[1])
        .filter((name) => !ARCHITECTURE_EXCLUSIONS.includes(name)),
    ),
  ];
  if (names.length !== ARCHITECTURE_NAME_COUNT)
    problems.push("architecture 5.1 이름 수 불일치");
  const range = /`space-1`~`space-12`/u.test(section);
  if (!range) problems.push("architecture 5.1 간격 범위 표기 오류");
  const ranges = range
    ? Array.from({ length: 12 }, (_, index) => `space-${index + 1}`)
    : [];
  return {
    names: [
      ...new Set([
        ...names.flatMap((name) => WILDCARDS[name] ?? [name]),
        ...ranges,
      ]),
    ],
    problems,
  };
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
  const documented = architectureNames(architecture);
  problems.push(...documented.problems);
  for (const name of documented.names) {
    if (!names.has(name))
      problems.push(`architecture 5.1 필수 토큰 누락: ${name}`);
  }
  for (const token of tokens.spacing.tokens) {
    const match = /^space-(\d+)$/u.exec(token.name);
    if (
      match !== null &&
      ["light", "dark"].some(
        (theme) => token.value[theme] !== `${Number(match[1]) * 4}px`,
      )
    )
      problems.push(`간격 배율 오류: ${token.name}`);
  }
  const order = (group, ordered, message) => {
    for (const theme of ["light", "dark"]) {
      const values = ordered.map((name) =>
        Number(
          group.tokens
            .find((token) => token.name === name)
            ?.value[theme]?.replace(/px$/u, ""),
        ),
      );
      if (
        values.some(
          (value, index) =>
            !Number.isFinite(value) ||
            (index > 0 && value <= values[index - 1]),
        )
      )
        problems.push(message);
    }
  };
  order(tokens.layout, ["bp-window-md", "bp-window-lg"], "창 구간 순서 오류");
  order(
    tokens.layout,
    ["bp-content-medium", "bp-content-wide"],
    "본문 구간 순서 오류",
  );
  order(tokens.zIndex, Z_ORDER, "겹침 순서 오류");
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
    if (!existsSync(resolve(ROOT, "design", font.file)))
      problems.push(`글꼴 파일 없음: ${font.file}`);
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
