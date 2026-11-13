import { readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { collectRules } from "../../apps/desktop/eslint/ds-snapshot.mjs";
export { collectRules } from "../../apps/desktop/eslint/ds-snapshot.mjs";
import {
  DS_CLASSES,
  DS_PROBES,
  DS_VARIANTS,
} from "../../apps/desktop/eslint/ds-classes.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const desktop = join(root, "apps/desktop");
const require = createRequire(join(desktop, "package.json"));
const {
  compile,
  __unstable__loadDesignSystem: loadDesignSystem,
} = require("tailwindcss");
const read = (p) => readFileSync(join(root, p), "utf8");
const generated = read("apps/desktop/src/shared/ds/tokens.css");
const theme =
  '@import "tailwindcss" source(none);\n@import "./tokens.css";\n@source "../../";\n';
export const cssBoundaryProblems = (name, source) =>
  name === "theme.css"
    ? source === theme
      ? []
      : ["theme entry changed"]
    : name === "tokens.css"
      ? []
      : ["handwritten CSS forbidden"];
export const snapshot = async (
  mode = "normal",
  classes = DS_CLASSES,
  variants = DS_VARIANTS,
) => {
  const css =
    mode === "without-reset"
      ? generated.replace("--*: initial;", "")
      : generated;
  const source = `${read("apps/desktop/node_modules/tailwindcss/theme.css")}\n${css}\n@tailwind utilities;`;
  const builder = await compile(source);
  const design = await loadDesignSystem(source);
  const allowed = [
    ...new Set([
      ...classes,
      ...variants.flatMap((v) => classes.map((c) => `${v}:${c}`)),
    ]),
  ].sort();
  const candidates = [...new Set([...allowed, ...DS_PROBES])].sort();
  const compiled = design.candidatesToCss(candidates);
  return {
    candidates: candidates.map((name, index) => ({
      name,
      kind: allowed.includes(name) ? "allowed" : "probe",
      generated: compiled[index] === null ? "no" : "yes",
    })),
    rules: collectRules(builder.build(candidates)),
  };
};
export const generationProblems = (value) =>
  value.candidates
    .filter((c) => c.kind === "allowed" && c.generated === "no")
    .map((c) => `ungenerated allowed class: ${c.name}`);
const cssFiles = (folder) =>
  readdirSync(folder, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory()
      ? cssFiles(join(folder, e.name))
      : e.name.endsWith(".css")
        ? [join(folder, e.name)]
        : [],
  );
export const check = async () => {
  const value = await snapshot();
  const problems = [...generationProblems(value)];
  if (
    JSON.stringify(value) !==
    JSON.stringify(JSON.parse(read("harness/ds-style/utilities.json")))
  )
    problems.push("Tailwind utility snapshot changed");
  for (const file of cssFiles(join(desktop, "src/shared/ds")))
    problems.push(
      ...cssBoundaryProblems(
        file.slice(file.lastIndexOf("/") + 1),
        readFileSync(file, "utf8"),
      ).map((p) => `${file}: ${p}`),
    );
  return problems;
};
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const problems = await check();
  problems.forEach((p) => process.stdout.write(`FAIL ${p}\n`));
  process.stdout.write(`ds-style: ${problems.length} failure(s)\n`);
  process.exitCode = problems.length === 0 ? 0 : 1;
}
