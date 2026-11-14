import {
  mkdtempSync,
  writeFileSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { build } from "vite";
import { MessageChannel } from "node:worker_threads";
import { JSDOM } from "jsdom";
import { desktopRoot } from "./paths.mjs";
export const componentsDom = async (source) => {
  const folder = mkdtempSync(join(desktopRoot, "node_modules/.ds03-dom-"));
  const instances = [];
  const channels = [];
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
          "STORY_ENTRY",
          JSON.stringify(join(desktopRoot, "src/shared/ds/stories.ts")),
        )
        .replaceAll(
          "INPUT_ENTRY",
          JSON.stringify(join(desktopRoot, "src/shared/platform/input.ts")),
        ),
    );
    await build({
      root: desktopRoot,
      configFile: false,
      logLevel: "error",
      define: { "process.env.NODE_ENV": '"development"' },
      build: {
        lib: { entry, name: "Controls", formats: ["iife"] },
        outDir: dist,
        emptyOutDir: true,
        minify: false,
      },
    });
    const script = readFileSync(
      join(
        dist,
        readdirSync(dist).find((file) => file.endsWith(".js")),
      ),
      "utf8",
    );
    return {
      open: () => {
        const dom = new JSDOM("<!doctype html><html><body></body></html>", {
          runScripts: "outside-only",
          pretendToBeVisual: true,
          url: "http://localhost/",
        });
        instances.push(dom);
        dom.window.MessageChannel = class extends MessageChannel {
          constructor() {
            super();
            channels.push(this);
          }
        };
        dom.window.IS_REACT_ACT_ENVIRONMENT = true;
        dom.window.eval(script);
        return dom.window;
      },
      close: () => {
        for (const dom of instances) {
          dom.window.cleanup();
          dom.window.close();
        }
        for (const channel of channels) {
          channel.port1.close();
          channel.port2.close();
        }
        rmSync(folder, { recursive: true, force: true });
      },
    };
  } catch (error) {
    rmSync(folder, { recursive: true, force: true });
    throw error;
  }
};
