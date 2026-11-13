import { cssProblems } from "../../apps/desktop/eslint/ds-css.mjs";
export { cssProblems, CSS_CONFIG } from "../../apps/desktop/eslint/ds-css.mjs";
import { readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import {
  DS_CLASSES,
  DS_PROBES,
  DS_VARIANTS,
} from "../../apps/desktop/eslint/ds-classes.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const desktop = join(root, "apps/desktop");
const require = createRequire(join(desktop, "package.json"));
const { compile } = require("tailwindcss");
const read = (p) => readFileSync(join(root, p), "utf8");
const generated = read("apps/desktop/src/shared/ds/tokens.css");
export const snapshot = async (mode = "normal") => {
  const css =
    mode === "without-reset"
      ? generated.replace("--*: initial;", "")
      : generated;
  const builder = await compile(
    `${read("apps/desktop/node_modules/tailwindcss/theme.css")}\n${css}\n@tailwind utilities;`,
  );
  const candidates = [
    ...DS_CLASSES,
    ...DS_PROBES,
    ...DS_VARIANTS.flatMap((v) => DS_CLASSES.map((c) => `${v}:${c}`)),
  ];
  const built = builder.build(candidates);
  return [
    ...new Set(
      [...built.matchAll(/^\.((?:\\.|[^\s{])+)\s*\{/gmu)].map((m) =>
        m[1]
          .replace(/\\([\da-f]{1,6})\s/giu, (_, h) =>
            String.fromCodePoint(parseInt(h, 16)),
          )
          .replace(/\\(.)/gu, "$1"),
      ),
    ),
  ].sort();
};
const cssFiles = (folder) =>
  readdirSync(folder, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory()
      ? cssFiles(join(folder, e.name))
      : e.name.endsWith(".css")
        ? [join(folder, e.name)]
        : [],
  );
export const check = async () => {
  const problems = [];
  const theme = read("apps/desktop/src/shared/ds/theme.css");
  if (
    theme !==
    '@import "tailwindcss" source(none);\n@import "./tokens.css";\n@source "../../";\n'
  )
    problems.push("theme entry changed");
  if (
    JSON.stringify(await snapshot()) !==
    JSON.stringify(JSON.parse(read("harness/ds-style/utilities.json")))
  )
    problems.push("Tailwind utility snapshot changed");
  for (const file of cssFiles(join(desktop, "src/shared/ds"))) {
    if (
      ["tokens.css", "theme.css"].some(
        (n) => file === join(desktop, "src/shared/ds", n),
      )
    )
      continue;
    problems.push(
      ...(await cssProblems(readFileSync(file, "utf8"))).map(
        (p) => `${file}: ${p}`,
      ),
    );
  }
  return problems;
};
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const problems = await check();
  problems.forEach((p) => process.stdout.write(`FAIL ${p}\n`));
  process.stdout.write(`ds-style: ${problems.length} failure(s)\n`);
  process.exitCode = problems.length === 0 ? 0 : 1;
}
