import { createRequire } from "node:module";
import { createServer } from "node:http";
import { readFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve, join, extname, sep } from "node:path";
import { createHash } from "node:crypto";
import { settings, environmentProblems, compareCaptures } from "./policy.mjs";
const root = resolve(import.meta.dirname, "../..");
const problems = environmentProblems({
  platform: process.platform,
  arch: process.arch,
  container: existsSync("/.dockerenv") ? "present" : "absent",
  image: process.env.VISUAL_IMAGE,
});
if (problems.length !== 0) throw new Error(problems.join("\n"));
const mode = process.argv[2];
if (!["measure", "update", "compare"].includes(mode))
  throw new Error("measure/update/compare 필요");
const require = createRequire(join(root, "apps/desktop/package.json"));
const { chromium } = require("@playwright/test");
const build = join(root, "target/DS-15/storybook");
const index = JSON.parse(readFileSync(join(build, "index.json"), "utf8"));
const types = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".woff2": "font/woff2",
  ".svg": "image/svg+xml",
};
const server = createServer((request, response) => {
  const file = resolve(
    build,
    `.${decodeURIComponent(new URL(request.url, "http://localhost").pathname)}`,
  );
  if (!file.startsWith(build + sep) || !existsSync(file)) {
    response.writeHead(404);
    response.end();
    return;
  }
  response.setHeader(
    "Content-Type",
    types[extname(file)] ?? "application/octet-stream",
  );
  response.end(readFileSync(file));
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const origin = `http://127.0.0.1:${server.address().port}`;
const output = join(root, "target/DS-15/captures");
const baselines = join(root, "harness/visual/baselines");
mkdirSync(output, { recursive: true });
const measurements = [];
try {
  const browser = await chromium.launch({ headless: true });
  try {
    for (const component of settings.components)
      for (const theme of settings.themes) {
        const entry = Object.values(index.entries).find(
          (entry) =>
            entry.type === "story" &&
            entry.importPath.endsWith(`/${component}.stories.mjs`) &&
            entry.exportName === (theme === "light" ? "Light0" : "Dark0"),
        );
        if (entry === undefined)
          throw new Error(`이야기 누락: ${component}/${theme}`);
        const captures = [];
        for (const pass of [1, 2]) {
          const context = await browser.newContext({
            viewport: settings.viewport,
            deviceScaleFactor: settings.scale,
            locale: settings.locale,
            timezoneId: settings.timezone,
            colorScheme: theme,
            reducedMotion: "reduce",
            serviceWorkers: "block",
          });
          try {
            await context.route("**/*", (route) =>
              new URL(route.request().url()).origin === origin
                ? route.continue()
                : route.abort(),
            );
            await context.addInitScript(
              ({ random }) => {
                Math.random = () => random;
              },
              { random: settings.random },
            );
            const page = await context.newPage();
            const errors = [];
            page.on("pageerror", (error) => errors.push(error.message));
            await page.clock.install({ time: new Date(settings.time) });
            await page.clock.pauseAt(new Date(settings.time));
            await page.goto(
              `${origin}/iframe.html?id=${encodeURIComponent(entry.id)}&viewMode=story`,
              { waitUntil: "networkidle" },
            );
            await page
              .locator(
                `[data-catalog-component="${component}"][data-catalog-theme="${theme}"]`,
              )
              .waitFor();
            await page.addStyleTag({
              content: `*, *::before, *::after { animation: none !important; transition: none !important; caret-color: transparent !important; } html, body { font-family: "IBM Plex Sans KR" !important; }`,
            });
            const fonts = await page.evaluate(async () => {
              await globalThis.document.fonts.ready;
              await Promise.all([
                globalThis.document.fonts.load('400 16px "IBM Plex Sans KR"'),
                globalThis.document.fonts.load('400 16px "IBM Plex Mono"'),
              ]);
              return [...globalThis.document.fonts]
                .filter((font) => font.status === "loaded")
                .map((font) => font.family);
            });
            if (
              !fonts.includes("IBM Plex Sans KR") ||
              !fonts.includes("IBM Plex Mono") ||
              errors.length !== 0
            )
              throw new Error(
                `글꼴/페이지 실패: ${JSON.stringify({ fonts, errors })}`,
              );
            const inputs = await page.evaluate(() => ({
              time: Date.now(),
              random: Math.random(),
            }));
            if (
              inputs.time !== Date.parse(settings.time) ||
              inputs.random !== settings.random
            )
              throw new Error("시계·난수 고정 실패");
            const session = await context.newCDPSession(page);
            await session.send("DOM.enable");
            await session.send("CSS.enable");
            const { root: dom } = await session.send("DOM.getDocument");
            const { nodeIds } = await session.send("DOM.querySelectorAll", {
              nodeId: dom.nodeId,
              selector: "[data-catalog-component], [data-catalog-component] *",
            });
            for (const nodeId of nodeIds) {
              const { fonts: used } = await session.send(
                "CSS.getPlatformFontsForNode",
                { nodeId },
              );
              if (
                used.some((font) => font.glyphCount > 0 && !font.isCustomFont)
              )
                throw new Error(`시스템 글꼴 사용: ${JSON.stringify(used)}`);
            }
            await session.detach();
            const bytes = await page.screenshot({
              animations: settings.animations,
              caret: settings.caret,
            });
            writeFileSync(
              join(output, `${component}-${theme}-${pass}.png`),
              bytes,
            );
            captures.push(bytes);
          } finally {
            await context.close();
          }
        }
        const differences = compareCaptures(...captures);
        if (differences.length !== 0)
          throw new Error(`${component}/${theme}: ${differences.join()}`);
        const name = `${component}-${theme}-chromium-linux-arm64.png`;
        if (mode === "update") {
          mkdirSync(baselines, { recursive: true });
          writeFileSync(join(baselines, name), captures[0]);
        }
        if (mode === "compare") {
          const baseline = existsSync(join(baselines, name))
            ? readFileSync(join(baselines, name))
            : undefined;
          if (compareCaptures(baseline, captures[0]).length !== 0)
            throw new Error(`기준 화면 불일치: ${name}`);
        }
        measurements.push({
          component,
          theme,
          bytes: captures[0].length,
          first: createHash("sha256").update(captures[0]).digest("hex"),
          second: createHash("sha256").update(captures[1]).digest("hex"),
        });
      }
    writeFileSync(
      join(output, "measurement.json"),
      JSON.stringify({ settings, measurements }, null, 2),
    );
    process.stdout.write(`${measurements.length} pairs byte-identical\n`);
  } finally {
    await browser.close();
  }
} finally {
  await new Promise((done) => server.close(done));
}
