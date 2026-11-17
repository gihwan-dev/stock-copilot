import { storedProblems, writeBaselineSet } from "./records.mjs";
import { captureStory } from "./capture.mjs";
import { staticServer } from "./server.mjs";
import { createRequire } from "node:module";
import { readFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";
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
const output = join(root, "target/DS-15/captures");
const baselines = join(root, "harness/visual/baselines");
if (mode === "compare") {
  const invalid = storedProblems(baselines);
  if (invalid.length) throw new Error(invalid.join("\n"));
}
const server = staticServer(build);
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const origin = `http://127.0.0.1:${server.address().port}`;
mkdirSync(output, { recursive: true });
const measurements = [];
const images = {};
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
          const bytes = await captureStory(browser, {
            origin,
            entry,
            component,
            theme,
          });
          writeFileSync(
            join(output, `${component}-${theme}-${pass}.png`),
            bytes,
          );
          captures.push(bytes);
        }
        const differences = compareCaptures(...captures);
        if (differences.length !== 0)
          throw new Error(`${component}/${theme}: ${differences.join()}`);
        const name = `${component}-${theme}-chromium-linux-arm64.png`;
        images[name] = captures[0];
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
    if (mode === "update")
      await writeBaselineSet(baselines, measurements, images);
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
