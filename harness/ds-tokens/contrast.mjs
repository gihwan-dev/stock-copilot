import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
const PAIR_COUNT = 65;
const PENDING_COUNT = 2;
const validName = (value) =>
  typeof value === "string" && /^[a-z\d-]+$/u.test(value);
const key = (entry) => `${entry.foreground}/${entry.background}`;

export const pairProblems = (contract) => {
  const problems = [];
  if (!Array.isArray(contract?.pairs) || contract.pairs.length !== PAIR_COUNT)
    return ["필수 대비 조합 수가 다릅니다"];
  if (!Array.isArray(contract.exclusions) || contract.exclusions.length !== 2)
    problems.push("색 제외 목록 수가 다릅니다");
  if (
    !Array.isArray(contract.pending) ||
    contract.pending.length !== PENDING_COUNT
  )
    problems.push("사용자 확인 대기 목록 수가 다릅니다");
  const seen = new Set();
  for (const pair of contract.pairs) {
    if (
      !validName(pair.foreground) ||
      !validName(pair.background) ||
      !(pair.minimum >= 1) ||
      typeof pair.basis !== "string" ||
      pair.basis.length === 0 ||
      Object.keys(pair).some(
        (k) =>
          ![
            "foreground",
            "background",
            "minimum",
            "darkMinimum",
            "basis",
          ].includes(k),
      )
    )
      problems.push("대비 조합 형식 오류");
    if (pair.darkMinimum !== undefined && !(pair.darkMinimum > 0))
      problems.push("다크 기준 오류");
    if (seen.has(key(pair))) problems.push(`중복 조합 ${key(pair)}`);
    seen.add(key(pair));
  }
  for (const entry of contract.exclusions ?? [])
    if (
      !validName(entry.token) ||
      typeof entry.reason !== "string" ||
      entry.reason.length === 0
    )
      problems.push("색 제외 사유 오류");
  const pendingKeys = new Set();
  for (const entry of contract.pending ?? []) {
    const pair = contract.pairs.find((p) => key(p) === key(entry));
    const identity = `${key(entry)}/${entry.theme}`;
    if (
      !pair ||
      !["light", "dark"].includes(entry.theme) ||
      entry.status !== "미달, 사용자 확인 대기" ||
      entry.basis !== pair.basis ||
      !Number.isFinite(entry.actual) ||
      pendingKeys.has(identity)
    )
      problems.push("확인 대기 항목 오류");
    pendingKeys.add(identity);
  }
  return problems;
};

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

export const contrast = (foreground, background) => {
  const back = parseColor(background);
  if (back[3] !== 1)
    throw new Error("반투명 배경은 대비 계약에서 지원하지 않습니다");
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

export const contrastProblems = (tokens, contract, report = () => {}) => {
  const problems = pairProblems(contract);
  if (problems.length > 0) return problems;
  const covered = new Set(
    contract.pairs.flatMap((pair) => [pair.foreground, pair.background]),
  );
  const excluded = new Set(contract.exclusions.map((entry) => entry.token));
  const names = new Set(tokens.color.tokens.map((token) => token.name));
  for (const name of names)
    if (!covered.has(name) && !excluded.has(name))
      problems.push(`대비 조합 없는 색: ${name}`);
  for (const name of excluded)
    if (!names.has(name) || covered.has(name))
      problems.push(`남아 있는 색 제외: ${name}`);
  const consumed = new Set();
  for (const theme of ["light", "dark"])
    for (const pair of contract.pairs) {
      try {
        const ratio = contrast(
          resolveColor(tokens, pair.foreground, theme),
          resolveColor(tokens, pair.background, theme),
        );
        const minimum =
          theme === "dark" ? (pair.darkMinimum ?? pair.minimum) : pair.minimum;
        const label = `${theme} ${key(pair)}`;
        const pending = contract.pending.find(
          (entry) => entry.theme === theme && key(entry) === key(pair),
        );
        report(
          `${label} ${ratio.toFixed(9)} >= ${minimum}${pending ? " 미달, 사용자 확인 대기" : ""}`,
        );
        if (pending) {
          consumed.add(pending);
          if (
            ratio >= minimum ||
            pending.minimum !== minimum ||
            Math.abs(ratio - pending.actual) > 1e-9
          )
            problems.push(`확인 대기 목록 갱신 필요: ${label} ${ratio}`);
        } else if (ratio < minimum)
          problems.push(`${label} ${ratio} < ${minimum}`);
      } catch (error) {
        problems.push(`${theme} ${key(pair)}: ${error.message}`);
      }
    }
  for (const entry of contract.pending)
    if (!consumed.has(entry))
      problems.push(`쓰이지 않은 확인 대기: ${key(entry)}`);
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
