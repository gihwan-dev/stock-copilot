import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";

const requiredPairs = () => [
  ...["ink", "ink-muted", "ink-subtle", "up", "down"].flatMap((foreground) =>
    ["bg", "surface", "surface-sunken"].map((background) => ({
      foreground,
      background,
      minimum: 4.5,
    })),
  ),
  ...["line-strong", "focus-ring"].flatMap((foreground) =>
    ["bg", "surface", "surface-sunken"].map((background) => ({
      foreground,
      background,
      minimum: 3,
    })),
  ),
  ...["primary", "up", "down", "alert"].flatMap((name) => [
    { foreground: `on-${name}`, background: name, minimum: 4.5 },
    { foreground: name, background: `${name}-soft`, minimum: 4.5 },
  ]),
  ...["primary", "up", "down"].map((name) => ({
    foreground: `on-${name}`,
    background: `${name}-hover`,
    minimum: 4.5,
  })),
  ...["chart-1", "chart-2", "chart-3", "chart-4", "chart-5", "chart-6"].map(
    (foreground) => ({ foreground, background: "surface", minimum: 3 }),
  ),
];

export const pairProblems = (pairs) =>
  isDeepStrictEqual(pairs, requiredPairs())
    ? []
    : ["필수 대비 조합 표가 닫힌 계약과 다릅니다"];

const parseColor = (value) => {
  if (/^#[\da-f]{6}$/iu.test(value))
    return [...value.slice(1).matchAll(/../gu)]
      .map((x) => Number.parseInt(x[0], 16))
      .concat(1);
  const rgba = /^rgba\((\d+), (\d+), (\d+), (0(?:\.\d+)?|1)\)$/u.exec(value);
  if (rgba !== null) {
    const channels = rgba.slice(1).map(Number);
    if (channels.slice(0, 3).every((c) => c >= 0 && c <= 255)) return channels;
  }
  throw new Error(`지원하지 않는 색: ${value}`);
};

const composite = (front, back) =>
  front
    .slice(0, 3)
    .map((c, i) => c * front[3] + back[i] * (1 - front[3]))
    .concat(1);
const luminance = (channels) => {
  const [r, g, b] = channels
    .slice(0, 3)
    .map((c) => c / 255)
    .map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return r * 0.2126 + g * 0.7152 + b * 0.0722;
};

export const contrast = (foreground, background, underlay) => {
  let back = parseColor(background);
  if (back[3] !== 1) {
    if (underlay === undefined)
      throw new Error("반투명 배경에는 불투명 underlay가 필요합니다");
    const base = parseColor(underlay);
    if (base[3] !== 1) throw new Error("underlay는 불투명해야 합니다");
    back = composite(back, base);
  }
  const front = composite(parseColor(foreground), back);
  const a = luminance(front),
    b = luminance(back);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
};

const resolveColor = (tokens, name, theme, visited = new Set()) => {
  if (visited.has(name)) throw new Error(`색 참조 순환: ${name}`);
  visited.add(name);
  const value = tokens.color.tokens.find((t) => t.name === name)?.value[theme];
  if (typeof value !== "string")
    throw new Error(`색이 없습니다: ${name}/${theme}`);
  const reference = /^\{([a-z\d-]+)\}$/u.exec(value);
  return reference === null
    ? value
    : resolveColor(tokens, reference[1], theme, visited);
};

export const contrastProblems = (tokens, pairs, report = () => {}) => {
  const problems = pairProblems(pairs);
  if (problems.length > 0) return problems;
  for (const theme of ["light", "dark"])
    for (const pair of pairs) {
      try {
        const ratio = contrast(
          resolveColor(tokens, pair.foreground, theme),
          resolveColor(tokens, pair.background, theme),
          pair.underlay === undefined
            ? undefined
            : resolveColor(tokens, pair.underlay, theme),
        );
        const label = `${theme} ${pair.foreground}/${pair.background}`;
        report(`${label} ${ratio.toFixed(6)} >= ${pair.minimum}`);
        if (ratio < pair.minimum)
          problems.push(`${label} ${ratio} < ${pair.minimum}`);
      } catch (error) {
        problems.push(
          `${theme} ${pair.foreground}/${pair.background}: ${error.message}`,
        );
      }
    }
  return problems;
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const tokens = JSON.parse(
      readFileSync(
        new URL(
          "../../apps/desktop/design-tokens/tokens.json",
          import.meta.url,
        ),
      ),
    );
    const pairs = JSON.parse(
      readFileSync(new URL("./contrast-pairs.json", import.meta.url)),
    );
    const problems = contrastProblems(tokens, pairs, (line) =>
      process.stdout.write(`${line}\n`),
    );
    problems.forEach((line) => process.stderr.write(`${line}\n`));
    process.stdout.write(`tokens-contrast: ${problems.length} failure(s)\n`);
    process.exitCode = problems.length === 0 ? 0 : 1;
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
