import {
  mkdtempSync,
  writeFileSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { build } from "vite";
import { chromium } from "playwright";
import { desktopRoot } from "./paths.mjs";
export const componentsBrowser = async (source) => {
  const folder = mkdtempSync(join(desktopRoot, "node_modules/.ds03-controls-"));
  let browser;
  try {
    const entry = join(folder, "entry.mjs"),
      dist = join(folder, "dist");
    writeFileSync(
      entry,
      source
        .replaceAll(
          "DS_ENTRY",
          JSON.stringify(join(desktopRoot, "src/shared/ds/index.ts")),
        )
        .replaceAll(
          "LIB_ENTRY",
          JSON.stringify(join(desktopRoot, "src/shared/lib/index.ts")),
        )
        .replaceAll(
          "PLATFORM_ENTRY",
          JSON.stringify(join(desktopRoot, "src/shared/platform/index.ts")),
        ) +
        `\nimport ${JSON.stringify(join(desktopRoot, "src/shared/ds/theme.css"))};`,
    );
    await build({
      root: desktopRoot,
      configFile: join(desktopRoot, "vite.config.mjs"),
      logLevel: "error",
      define: { "process.env.NODE_ENV": '"development"' },
      build: {
        lib: { entry, name: "Controls", formats: ["iife"] },
        outDir: dist,
        emptyOutDir: true,
        minify: false,
      },
    });
    const files = readdirSync(dist);
    const script = readFileSync(
        join(
          dist,
          files.find((f) => f.endsWith(".js")),
        ),
        "utf8",
      ),
      css = readFileSync(
        join(
          dist,
          files.find((f) => f.endsWith(".css")),
        ),
        "utf8",
      );
    browser = await chromium.launch({ headless: true });
    return {
      page: async () => {
        const page = await browser.newPage();
        await page.setContent(
          "<!doctype html><html><head></head><body></body></html>",
        );
        await page.addStyleTag({ content: css });
        await page.addScriptTag({ content: script });
        return page;
      },
      close: async () => {
        await browser.close();
        rmSync(folder, { recursive: true, force: true });
      },
    };
  } catch (error) {
    await browser?.close();
    rmSync(folder, { recursive: true, force: true });
    throw error;
  }
};
