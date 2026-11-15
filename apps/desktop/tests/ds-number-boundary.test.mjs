import assert from "node:assert/strict";
import test from "node:test";
import {
  cpSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { join, dirname } from "node:path";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";

test("DS05 reader access, conversion and price color boundaries reject violations", async () => {
  const folder = mkdtempSync("ds05-boundary-");
  try {
    mkdirSync(join(folder, "src/shared"), { recursive: true });
    cpSync(
      join(desktopRoot, "src/shared/lib"),
      join(folder, "src/shared/lib"),
      { recursive: true },
    );
    const project = join(folder, "tsconfig.json");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        compilerOptions: { rootDir: "src" },
        include: ["src"],
      }),
    );
    const engine = new ESLint({
      cwd: folder,
      overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
      overrideConfig: [
        {
          languageOptions: {
            parserOptions: {
              projectService: false,
              project,
              tsconfigRootDir: folder,
            },
          },
          settings: { "boundaries/root-path": folder },
        },
      ],
    });
    const cases = JSON.parse(
      readFileSync(
        new URL(
          "../../../harness/eslint-violations/ds05-number-display.json",
          import.meta.url,
        ),
        "utf8",
      ),
    );
    for (const entry of cases) {
      const source = join(folder, entry.file);
      mkdirSync(dirname(source), { recursive: true });
      writeFileSync(source, entry.code);
      const results = (await engine.lintFiles([source]))[0].messages;
      if (entry.rule === null)
        assert.equal(
          results.filter((m) => m.ruleId === "local/number-display-boundary")
            .length,
          0,
          JSON.stringify(results),
        );
      else
        assert.ok(
          results.some((m) => m.ruleId === entry.rule),
          JSON.stringify({ entry, results }),
        );
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
