import { createRequire } from "node:module";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { environmentProblems, settings } from "../visual/policy.mjs";

const root = resolve(import.meta.dirname, "../..");
const desktop = resolve(root, "apps/desktop");
const require = createRequire(resolve(desktop, "package.json"));
const { playwright } = await import(
  pathToFileURL(require.resolve("@vitest/browser-playwright"))
);
const pageErrors = [];
const first = Number(process.env.DS20_FIRST ?? "0");
const size = Number(process.env.DS20_SIZE ?? "30");
if (
  !Number.isSafeInteger(first) ||
  first < 0 ||
  !Number.isSafeInteger(size) ||
  size < 1
)
  throw new Error("측정 범위 오류");
const problems = environmentProblems({
  platform: process.platform,
  arch: process.arch,
  container: existsSync("/.dockerenv") ? "present" : "absent",
  image: process.env.VISUAL_IMAGE,
});
if (problems.length !== 0) throw new Error(problems.join("\n"));

export default {
  root: desktop,
  cacheDir: resolve(root, "target/DS-20/vite-cache"),
  publicDir: resolve(root, "target/DS-20/storybook"),
  optimizeDeps: { include: ["zod"] },
  define: {
    __DS20_FIRST__: JSON.stringify(first),
    __DS20_SIZE__: JSON.stringify(size),
  },
  server: { fs: { allow: [root] } },
  test: {
    include: [
      resolve(root, "apps/desktop/tests/ds20-overflow.browser.ts"),
      resolve(root, "harness/ds-overflow/catalog.browser.ts"),
    ],
    globals: true,
    fileParallelism: false,
    attachmentsDir: resolve(root, "target/DS-20/attachments"),
    allowOnly: false,
    passWithNoTests: false,
    expect: { requireAssertions: true },
    browser: {
      enabled: true,
      headless: true,
      commands: {
        prepareCatalog: async ({ context, page }) => {
          const origin = new URL(page.url()).origin;
          page.on("pageerror", (error) => pageErrors.push(error.message));
          await context.route("**/*", (route) =>
            new URL(route.request().url()).origin === origin
              ? route.continue()
              : route.abort(),
          );
          await page.addInitScript(
            ({ time, random }) => {
              const OriginalDate = Date;
              const FixedDate = function (...args) {
                if (!new.target) return new OriginalDate(time).toString();
                return Reflect.construct(
                  OriginalDate,
                  args.length === 0 ? [time] : args,
                  new.target,
                );
              };
              Object.setPrototypeOf(FixedDate, OriginalDate);
              FixedDate.prototype = OriginalDate.prototype;
              FixedDate.now = () => time;
              globalThis.Date = FixedDate;
              Math.random = () => random;
            },
            { time: Date.parse(settings.time), random: settings.random },
          );
        },
        catalogErrors: async () => pageErrors,
      },
      provider: playwright({
        contextOptions: {
          viewport: settings.viewport,
          deviceScaleFactor: settings.scale,
          locale: settings.locale,
          timezoneId: settings.timezone,
          reducedMotion: "reduce",
        },
      }),
      instances: [{ browser: settings.engine }],
    },
  },
};
