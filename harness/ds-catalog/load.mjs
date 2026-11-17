import { mkdtempSync, readdirSync, writeFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { createRequire } from "node:module";
export const root = resolve(import.meta.dirname, "../..");
export const desktop = join(root, "apps/desktop");
const require = createRequire(join(desktop, "package.json"));
export const loadCatalog = async () => {
  const { build } = await import(pathToFileURL(require.resolve("vite")).href);
  const folder = mkdtempSync(join(desktop, "node_modules/.ds07-"));
  try {
    const files = readdirSync(join(desktop, ".storybook/catalog")).filter(
      (file) => file.endsWith(".stories.mjs"),
    );
    const source = `import * as components from ${JSON.stringify(join(desktop, "src/shared/ds/index.ts"))};\nimport * as sources from ${JSON.stringify(join(desktop, "src/shared/ds/stories.ts"))};\nimport {stressComponentStories} from ${JSON.stringify(join(desktop, "src/shared/ds/testing/Stress.stories.tsx"))};\nimport {catalogSamples} from ${JSON.stringify(join(import.meta.dirname, "samples.mjs"))};\nexport {components}; export const samples=catalogSamples({...sources,stressComponentStories});\n${files.map((file, index) => `import * as page${index} from ${JSON.stringify(join(desktop, ".storybook/catalog", file))};`).join("\n")}\nexport const modules={${files.map((file, index) => `${JSON.stringify(file)}:page${index}`).join(",")}};`;
    const entry = join(folder, "entry.mjs");
    writeFileSync(entry, source);
    await build({
      root: desktop,
      configFile: false,
      logLevel: "error",
      build: {
        ssr: entry,
        outDir: join(folder, "out"),
        rollupOptions: { output: { entryFileNames: "entry.mjs" } },
      },
    });
    return await import(pathToFileURL(join(folder, "out/entry.mjs")).href);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
};
