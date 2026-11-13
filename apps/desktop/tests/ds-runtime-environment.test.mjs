import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  rmSync,
  writeFileSync,
  readFileSync,
  realpathSync,
} from "node:fs";
import { join, relative } from "node:path";
import { mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";

test("runtime lint keeps real React exports after repeated source replacement in CI", () => {
  const folder = mkdtempSync(join(desktopRoot, ".ds-ci-regression-"));
  try {
    assert.ok(relative(desktopRoot, folder).startsWith(".."));
    assert.equal(
      realpathSync(join(folder, "node_modules")),
      realpathSync(join(desktopRoot, "node_modules")),
    );
    assert.equal(
      JSON.parse(readFileSync(join(folder, "package.json"), "utf8")).type,
      "module",
    );
    mkdirSync(join(folder, "src/shared/ds"), { recursive: true });
    const project = join(folder, "tsconfig.json"),
      source = join(folder, "src/shared/ds/Probe.tsx");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        compilerOptions: { rootDir: "src" },
        include: ["src"],
      }),
    );
    const code = `
      import assert from "node:assert/strict";
      import { writeFileSync } from "node:fs";
      import { ESLint } from ${JSON.stringify(new URL("./ds-runtime-environment.mjs", import.meta.url).href)};
      const source = ${JSON.stringify(source)};
      const probe = {
        rules: {
          exports: {
            meta: { schema: [] },
            create(context) {
              return {
                Program() {
                  const program = context.sourceCode.parserServices.program;
                  assert.notEqual(program.getCompilerOptions().noResolve, true);
                  const checker = program.getTypeChecker();
                  const file = program.getSourceFile(source);
                  const imported = file.statements.find(s => s.moduleSpecifier?.text === "react");
                  const symbol = checker.getSymbolAtLocation(imported.moduleSpecifier);
                  const exports = checker.getExportsOfModule(symbol).map(s => s.name);
                  assert.ok(exports.includes("ReactElement"), JSON.stringify(exports));
                },
              };
            },
          },
        },
      };
      const engine = new ESLint({
        cwd: ${JSON.stringify(folder)},
        overrideConfigFile: ${JSON.stringify(join(desktopRoot, "eslint.config.mjs"))},
        overrideConfig: [{
          languageOptions: {
            parserOptions: {
              projectService: false,
              project: ${JSON.stringify(project)},
              tsconfigRootDir: ${JSON.stringify(folder)},
            },
          },
          settings: { "boundaries/root-path": ${JSON.stringify(folder)} },
          plugins: { probe },
          rules: { "probe/exports": "error" },
        }],
      });
      for (const text of [
        'import type {ReactElement} from "react"; export const Probe=():ReactElement=><div className="bg-primary"/>;',
        'import {useRef} from "react"; import type {ReactElement} from "react"; export const Probe=():ReactElement=>{const ref=useRef<HTMLDivElement>(null); return <div ref={ref}/>;};'
      ]) {
        writeFileSync(source, text);
        const [result] = await engine.lintFiles([source]);
        assert.deepEqual(result.messages, []);
      }
    `;
    const run = spawnSync(
      process.execPath,
      ["--input-type=module", "-e", code],
      { env: { ...process.env, CI: "true" }, encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stdout + run.stderr);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
