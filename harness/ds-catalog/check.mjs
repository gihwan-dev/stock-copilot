import { mkdtempSync, readdirSync, writeFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { createRequire } from "node:module";
import { catalogProblems } from "./rules.mjs";
const root = resolve(import.meta.dirname, "../..");
const desktop = join(root, "apps/desktop");
const require = createRequire(join(desktop, "package.json"));
const { build } = await import(pathToFileURL(require.resolve("vite")).href);
const folder = mkdtempSync(join(desktop, "node_modules/.ds07-"));
try {
  const files = readdirSync(join(desktop, ".storybook/catalog")).filter(
    (file) => file.endsWith(".stories.mjs"),
  );
  const source = `import * as components from ${JSON.stringify(join(desktop, "src/shared/ds/index.ts"))};\nexport { components };\nimport * as sources from ${JSON.stringify(join(desktop, "src/shared/ds/stories.ts"))};\nexport const catalogStories=Object.values(sources).flat();\n${files.map((file, index) => `import * as page${index} from ${JSON.stringify(join(desktop, ".storybook/catalog", file))};`).join("\n")}\nexport const entries = [${files.map((file, index) => `...[page${index}.Light,page${index}.Dark].map(page=>({component:${JSON.stringify(file.replace(".stories.mjs", ""))},theme:page?.parameters?.theme,render:page?.render}))`).join(",")}];`;
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
  const loaded = await import(
    pathToFileURL(join(folder, "out/entry.mjs")).href
  );
  const problems = catalogProblems(
    Object.keys(loaded.components),
    loaded.catalogStories,
    loaded.entries,
  );
  problems.forEach((problem) => process.stderr.write(`FAIL ${problem}\n`));
  process.stdout.write(
    `ds-catalog: ${Object.keys(loaded.components).length} components, ${loaded.catalogStories.length} samples, ${loaded.entries.length} theme pages, ${problems.length} problems\n`,
  );
  process.exitCode = problems.length === 0 ? 0 : 1;
} finally {
  rmSync(folder, { recursive: true, force: true });
}
