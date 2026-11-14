import assert from "node:assert/strict";
import test from "node:test";
import {
  readFileSync,
  writeFileSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  symlinkSync,
} from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { compile } from "tailwindcss";
import ts from "typescript";
import { verifyIndependent } from "./ds-independent.mjs";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const read = (path) => readFileSync(join(root, path), "utf8");
const cssPath = "apps/desktop/src/shared/ds/tokens.css";
const tsPath = "apps/desktop/src/shared/ds/tokens.ts";
const inputPath = "apps/desktop/design-tokens/tokens.json";
const cases = JSON.parse(read("harness/ds-tokens/utility-cases.json"));
const selector = (name) => `.${name.replace(/[@:]/gu, "\\$&")} {`;
const generator = (cwd, args = []) =>
  spawnSync(
    "cargo",
    [
      "run",
      "--locked",
      "--quiet",
      "--manifest-path",
      join(root, "Cargo.toml"),
      "--package",
      "xtask",
      "--",
      "gen-tokens",
      ...args,
    ],
    { cwd, encoding: "utf8" },
  );

const utilityProblems = async (source, entries) => {
  const built = (
    await compile(
      `${read("apps/desktop/node_modules/tailwindcss/theme.css")}\n${source}\n@tailwind utilities;`,
    )
  ).build(
    entries.map((c) => c.utility).concat(["bg-red-500", "p-5", "text-sm"]),
  );
  const problems = entries.flatMap((c) => [
    ...(source.replace(/\s+/gu, " ").includes(`${c.variable}: ${c.value};`)
      ? []
      : [`${c.name} variable/value`]),
    ...(built.includes(selector(c.utility)) ? [] : [`${c.name} utility`]),
  ]);
  for (const forbidden of ["bg-red-500", "p-5", "text-sm"])
    if (built.includes(selector(forbidden))) problems.push(forbidden);
  return problems;
};

test("Tailwind 4.3.5 compiles every token mapping and rejects defaults with per-element mutations", async () => {
  assert.equal(
    JSON.parse(read("apps/desktop/node_modules/tailwindcss/package.json"))
      .version,
    "4.3.5",
  );
  const source = read(cssPath);
  const tokens = JSON.parse(read(inputPath));
  const names = Object.values(tokens)
    .flatMap((g) => (g.tokens ?? []).map((t) => t.name))
    .concat(
      Object.keys(tokens.type.families),
      tokens.type.groups.flatMap((g) => g.styles.map((s) => s.name)),
    )
    .sort();
  assert.deepEqual(cases.map((c) => c.name).sort(), names);
  assert.deepEqual(await utilityProblems(source, cases), []);
  for (const entry of cases) {
    const changed = source
      .replace(/\s+/gu, " ")
      .replaceAll(`${entry.variable}: ${entry.value};`, "");
    assert.ok((await utilityProblems(changed, [entry])).length > 0, entry.name);
  }
  assert.ok(
    (
      await utilityProblems(source.replace("--*: initial;", ""), cases)
    ).includes("bg-red-500"),
  );
});

test("generation check rejects hand edits to either output and input without regeneration", () => {
  mkdirSync(join(root, "target"), { recursive: true });
  const scratch = mkdtempSync(join(root, "target", "ds-generation-"));
  try {
    symlinkSync(join(root, "node_modules"), join(scratch, "node_modules"));
    mkdirSync(dirname(join(scratch, inputPath)), { recursive: true });
    writeFileSync(join(scratch, inputPath), read(inputPath));
    const run = generator(scratch);
    assert.equal(run.status, 0, run.stderr);
    verifyIndependent(
      readFileSync(join(scratch, cssPath), "utf8"),
      JSON.parse(read(inputPath)),
    );
    assert.equal(generator(scratch, ["--check"]).status, 0);
    for (const path of [cssPath, tsPath]) {
      const original = readFileSync(join(scratch, path), "utf8");
      writeFileSync(join(scratch, path), original + "\n");
      assert.notEqual(generator(scratch, ["--check"]).status, 0, path);
      assert.equal(readFileSync(join(scratch, path), "utf8"), original + "\n");
      writeFileSync(join(scratch, path), original);
    }
    const changed = JSON.parse(read(inputPath));
    changed.spacing.tokens[0].value.light = "8px";
    writeFileSync(join(scratch, inputPath), JSON.stringify(changed));
    assert.notEqual(generator(scratch, ["--check"]).status, 0);
    writeFileSync(join(scratch, inputPath), "{}");
    assert.notEqual(generator(scratch).status, 0);
    assert.notEqual(generator(scratch, ["--unknown"]).status, 0);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});

test("removing a token name makes a typed consumer fail to compile", () => {
  const source = read(tsPath);
  const options = {
    strict: true,
    noEmit: true,
    skipLibCheck: true,
    target: ts.ScriptTarget.ESNext,
    module: ts.ModuleKind.ESNext,
  };
  const diagnose = (text) => {
    const host = ts.createCompilerHost(options);
    const name = join(root, "target", "token-types.ts");
    const original = host.getSourceFile;
    host.getSourceFile = (path, version, onError, fresh) =>
      path === name
        ? ts.createSourceFile(path, text, version, true)
        : original(path, version, onError, fresh);
    return ts
      .getPreEmitDiagnostics(ts.createProgram([name], options, host))
      .filter((d) => d.category === ts.DiagnosticCategory.Error);
  };
  const consumer =
    '\nconst selected: SpacingToken = "space-4";\nvoid selected;';
  assert.deepEqual(diagnose(source + consumer), []);
  assert.ok(
    diagnose(source.replace(/\s*\| "space-4"/u, "") + consumer).some(
      (d) => d.code === 2820 || d.code === 2322,
    ),
  );
});

test("CSS entry is confined to DS and Vite bundles the six protected fonts with OFL", () => {
  assert.equal(
    read("apps/desktop/src/shared/ds/theme.css"),
    '@import "tailwindcss" source(none);\n@import "./tokens.css";\n@source "../../";\n\n@utility rotate-disclosure {\n  transform: rotate(90deg);\n}\n\n@utility w-switch {\n  width: calc(var(--spacing-icon-md) * 2);\n}\n\n@utility border-t-hairline {\n  border-top-width: var(--border-width-hairline);\n}\n\n@utility stroke-icon {\n  stroke-width: var(--border-width-focus);\n}\n\n@utility stroke-linecap-round {\n  stroke-linecap: round;\n}\n\n@utility stroke-linejoin-round {\n  stroke-linejoin: round;\n}\n\n@utility bottom-border-hairline {\n  bottom: var(--border-width-hairline);\n}\n\n@utility h-border-hairline {\n  height: var(--border-width-hairline);\n}\n\n@utility left-border-hairline {\n  left: var(--border-width-hairline);\n}\n\n@utility ml-border-hairline {\n  margin-left: var(--border-width-hairline);\n}\n\n@utility size-border-hairline {\n  width: var(--border-width-hairline);\n}\n\n@utility size-border-hairline {\n  height: var(--border-width-hairline);\n}\n\n@utility top-border-hairline {\n  top: var(--border-width-hairline);\n}\n\n@utility w-border-focus {\n  width: var(--border-width-focus);\n}\n\n@utility w-border-hairline {\n  width: var(--border-width-hairline);\n}\n',
  );
  assert.match(
    read("apps/desktop/index.html"),
    /href="\/src\/shared\/ds\/theme\.css"/u,
  );
  assert.equal(read("apps/desktop/dist/OFL.txt"), read("design/fonts/OFL.txt"));
  const css = read(cssPath);
  for (const font of JSON.parse(read(inputPath)).type.fonts) {
    assert.ok(css.includes(`../../../../../design/${font.file}`));
  }
  assert.ok(!css.includes("https://"));
});
