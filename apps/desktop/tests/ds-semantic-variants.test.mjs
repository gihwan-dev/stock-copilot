import assert from "node:assert/strict";
import { test } from "node:test";
import {
  cpSync,
  writeFileSync,
  readFileSync,
  rmSync,
  mkdirSync,
} from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
const cases = JSON.parse(
  readFileSync(
    new URL(
      "../../../harness/eslint-violations/ds04-semantic.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
test("DS04 semantic unions require complete, local, consumed static token mappings", async () => {
  const folder = mkdtempSync("ds04-semantic-");
  try {
    cpSync(join(desktopRoot, "src"), join(folder, "src"), { recursive: true });
    const project = join(folder, "tsconfig.json"),
      source = join(folder, "src/shared/ds/Probe.tsx");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        compilerOptions: {
          rootDir: "src",
          noUnusedLocals: false,
          noUnusedParameters: false,
        },
        include: ["src"],
      }),
    );
    const config = {
      settings: { "boundaries/root-path": folder },
      languageOptions: {
        parserOptions: {
          projectService: false,
          project,
          tsconfigRootDir: folder,
        },
      },
    };
    const base = new ESLint({
      cwd: folder,
      overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
      overrideConfig: [config],
    });
    const resolved = await base.calculateConfigForFile(source);
    const engine = new ESLint({
      cwd: folder,
      overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
      overrideConfig: [
        config,
        {
          rules: {
            ...Object.fromEntries(
              Object.keys(resolved.rules).map((key) => [key, "off"]),
            ),
            "ds/public-props": "error",
            "ds/static-classes": "error",
          },
        },
      ],
    });
    for (const entry of cases) {
      for (const [name, code] of Object.entries(entry.files)) {
        const file = join(folder, "src/shared/ds", name);
        mkdirSync(join(file, ".."), { recursive: true });
        writeFileSync(file, code);
      }
      writeFileSync(
        source,
        'import type {ReactElement} from "react";' + entry.code,
      );
      const [result] = await engine.lintFiles([source]);
      assert.deepEqual(
        [...new Set(result.messages.map((message) => message.ruleId))].sort(),
        entry.expected.toSorted(),
        entry.name + JSON.stringify(result.messages),
      );
      if (entry.expected.length === 0) {
        const compiled = spawnSync(
          join(desktopRoot, "node_modules/.bin/tsc"),
          ["-p", project],
          { encoding: "utf8" },
        );
        assert.equal(
          compiled.status,
          0,
          entry.name + compiled.stdout + compiled.stderr,
        );
      }
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
