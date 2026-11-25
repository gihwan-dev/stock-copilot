import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { staticServer } from "../visual/server.mjs";
import { buildAudit } from "./build.mjs";
import { inspectWindowFill } from "./window-fill.mjs";

const root = resolve(import.meta.dirname, "../..");
const require = createRequire(join(root, "apps/desktop/package.json"));

export const sizes = Object.freeze({
  wide: { width: 1440, height: 900 },
  medium: { width: 1024, height: 700 },
  minimum: { width: 960, height: 640 },
  tall: { width: 1440, height: 2600 },
});
export const checkedSizes = Object.freeze(["wide", "medium", "minimum"]);
export const themes = Object.freeze(["light", "dark"]);

export const launchBrowser = async () => {
  const { chromium } = require("@playwright/test");
  try {
    return await chromium.launch({ headless: true });
  } catch {
    return chromium.launch({ headless: true, channel: "chrome" });
  }
};

const settle = async (page) => {
  await page.waitForFunction(() => globalThis.auditState !== undefined, null, {
    timeout: 15000,
  });
  const state = await page.evaluate(() => globalThis.auditState);
  if (state !== "ready") throw new Error(state);
  await page.evaluate(() => globalThis.document.fonts.ready);
  await page.evaluate(
    () =>
      new Promise((done) =>
        globalThis.requestAnimationFrame(() =>
          globalThis.requestAnimationFrame(done),
        ),
      ),
  );
};

export const inspectPage = (page) =>
  page.evaluate(`(${inspectWindowFill.toString()})(globalThis.document)`);

export const withAuditServer = async (work) => {
  const output = join(root, "target/screen-audit/dist");
  const html = await buildAudit(output);
  const server = staticServer(output);
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  const origin = `http://127.0.0.1:${server.address().port}`;
  const browser = await launchBrowser();
  try {
    return await work({ browser, url: `${origin}${html}` });
  } finally {
    await browser.close();
    await new Promise((done) => server.close(done));
  }
};

export const auditScenario = async (
  { browser, url },
  { scenario, size, theme, shot },
) => {
  const context = await browser.newContext({
    viewport: sizes[size],
    colorScheme: theme,
    locale: "ko-KR",
    timezoneId: "Asia/Seoul",
    reducedMotion: "reduce",
  });
  try {
    await context.route("**/*", (route) =>
      new URL(route.request().url()).origin === new URL(url).origin
        ? route.continue()
        : route.abort(),
    );
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(`${url}?s=${encodeURIComponent(scenario)}`);
    await settle(page);
    const findings = await inspectPage(page);
    if (shot !== undefined) await page.screenshot({ path: shot });
    return [
      ...findings,
      ...errors.map((detail) => ({ kind: "page-error", detail })),
    ];
  } finally {
    await context.close();
  }
};

export const scenarioIndex = async ({ browser, url }) => {
  const page = await browser.newPage();
  try {
    await page.goto(url);
    await page.waitForFunction(() => globalThis.auditState === "ready");
    return await page.evaluate(() => ({
      names: globalThis.auditNames,
      paths: globalThis.auditPaths,
    }));
  } finally {
    await page.close();
  }
};

export const auditWith = async (target, options = {}) => {
  const names = options.scenarios ?? (await scenarioIndex(target)).names;
  const results = [];
  for (const scenario of names)
    for (const size of options.sizes ?? checkedSizes)
      for (const theme of options.themes ?? themes) {
        const shot =
          options.shots === undefined
            ? undefined
            : join(options.shots, `${scenario}-${size}-${theme}.png`);
        const findings = await auditScenario(target, {
          scenario,
          size,
          theme,
          shot,
        }).catch((error) => [
          { kind: "scenario-error", detail: error.message },
        ]);
        results.push({ scenario, size, theme, findings });
      }
  return results;
};

export const auditAll = (options = {}) =>
  withAuditServer((target) => auditWith(target, options));

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const label = process.argv[2] ?? "current";
  const shots = join(root, "target/screen-audit", label);
  mkdirSync(shots, { recursive: true });
  const results = await auditAll({
    shots,
    sizes: ["wide", "medium", "tall"],
  });
  const failing = results.filter((row) => row.findings.length !== 0);
  for (const row of failing)
    process.stdout.write(
      `${row.scenario} ${row.size} ${row.theme}: ${JSON.stringify(row.findings)}\n`,
    );
  process.stdout.write(
    `screen-audit: ${results.length} 화면, 위반 ${failing.length}, 스크린샷 ${shots}\n`,
  );
  process.exitCode = failing.length === 0 ? 0 : 1;
}
