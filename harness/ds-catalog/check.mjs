import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, basename } from "node:path";
import { spawnSync } from "node:child_process";
import { desktop, loadCatalog } from "./load.mjs";
import { stylesheetProblems } from "./styles.mjs";
import { generationProblems } from "./generation.mjs";
import {
  catalogProblems,
  stressProblems,
  stressRenderProblems,
} from "./rules.mjs";
const folder = mkdtempSync(join(desktop, "node_modules/.ds07-index-"));
try {
  const run = spawnSync(
    join(desktop, "node_modules/.bin/storybook"),
    [
      "build",
      "--test",
      "--disable-telemetry",
      "--output-dir",
      join(folder, "storybook"),
    ],
    { cwd: desktop, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
  );
  if (run.status !== 0)
    throw new Error(`Storybook 빌드 실패\n${run.stdout}\n${run.stderr}`);
  const index = JSON.parse(
    readFileSync(join(folder, "storybook/index.json"), "utf8"),
  );
  const loaded = await loadCatalog();
  const stress = stressProblems(
    Object.keys(loaded.components),
    loaded.protectedStressStories,
  );
  stress.push(...stressRenderProblems(loaded.protectedStressStories));
  if (stress.length !== 0) throw new Error(stress.join("\n"));
  const entries = Object.values(index.entries)
    .filter((entry) => entry.type === "story")
    .map((item) => {
      const prefix = "./.storybook/catalog/";
      const module = item.importPath.startsWith(prefix)
        ? loaded.modules[item.importPath.slice(prefix.length)]
        : undefined;
      const page = module?.[item.exportName];
      const rendered =
        typeof page?.render === "function" ? page.render() : undefined;
      return {
        component: basename(item.importPath, ".stories.mjs"),
        theme: page?.parameters?.theme,
        tags: item.tags,
        classes: [
          rendered?.props?.className,
          rendered?.props?.children?.props?.className,
        ]
          .filter((value) => typeof value === "string")
          .flatMap((value) => value.split(/\s+/)),
        sample: page?.parameters?.sample,
        title: item.title.split("/").at(-1),
        render: page?.render,
        observedComponent: rendered?.props?.["data-catalog-component"],
        observedTheme: rendered?.props?.["data-catalog-theme"],
        observedSample: rendered?.props?.["data-catalog-sample"],
        observedRender:
          rendered?.props?.children?.props?.children?.props?.render,
      };
    });
  const problems = [
    ...stylesheetProblems(join(folder, "storybook"), [
      ...new Set(entries.flatMap((entry) => entry.classes)),
    ]),
    ...(await generationProblems(loaded.samples)),
    ...catalogProblems(Object.keys(loaded.components), loaded.samples, entries),
    ...stress,
  ];
  mkdirSync(join(desktop, "../../target/DS-07"), { recursive: true });
  writeFileSync(
    join(desktop, "../../target/DS-07/index-verdict.json"),
    JSON.stringify(
      {
        components: Object.keys(loaded.components),
        samples: loaded.samples.length,
        pages: entries.length,
        problems,
      },
      null,
      2,
    ),
  );
  problems.forEach((problem) => process.stderr.write(`FAIL ${problem}\n`));
  process.stdout.write(
    `ds-catalog: ${Object.keys(loaded.components).length} components, ${loaded.samples.length} samples, ${entries.length} indexed theme pages, ${problems.length} problems\n`,
  );
  process.exitCode = problems.length === 0 ? 0 : 1;
} finally {
  rmSync(folder, { recursive: true, force: true });
}
