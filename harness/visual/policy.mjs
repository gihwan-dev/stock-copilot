export const settings = Object.freeze({
  image:
    "mcr.microsoft.com/playwright:v1.65.0-noble@sha256:9beb546099e11f52d62e505537914870e01a67e0e6b00ae41da4775324e73dbe",
  engine: "chromium",
  viewport: Object.freeze({ width: 960, height: 640 }),
  scale: 1,
  locale: "ko-KR",
  timezone: "Asia/Seoul",
  time: "2026-01-02T03:04:05.000Z",
  random: 0.3141592653589793,
  animations: "disabled",
  caret: "hide",
  components: Object.freeze(["Button", "Card", "Money", "TextField"]),
  themes: Object.freeze(["light", "dark"]),
});
export const baselineProblems = (paths) =>
  paths
    .filter((path) => /-darwin(?:[-.][^/]*)?\.(png|jpe?g|webp)$/i.test(path))
    .map((path) => `호스트 기준 화면 금지: ${path}`);
export const environmentProblems = (environment) =>
  Object.entries({
    platform: "linux",
    arch: "arm64",
    container: "present",
    image: settings.image,
  }).flatMap(([key, value]) =>
    environment[key] === value ? [] : [`기준 환경 불일치: ${key}`],
  );
export const compareCaptures = (first, second) =>
  first === undefined ||
  second === undefined ||
  first.length === 0 ||
  !first.equals(second)
    ? ["기준 화면 바이트 불일치 또는 누락"]
    : [];
