import assert from "node:assert/strict";
import { test } from "node:test";
import {
  cpSync,
  existsSync,
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
      settings: {
        "boundaries/root-path": folder,
        "ds/price-color-files": ["src/shared/ds/Badge.tsx"],
      },
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
      for (const [name, code] of Object.entries(entry.appendFiles ?? {})) {
        writeFileSync(
          join(folder, "src/shared/ds", name),
          readFileSync(join(desktopRoot, "src/shared/ds", name), "utf8") + code,
        );
      }
      const target = entry.sourcePath
        ? join(folder, "src/shared/ds", entry.sourcePath)
        : source;
      mkdirSync(join(target, ".."), { recursive: true });
      writeFileSync(
        target,
        'import type {ReactElement} from "react";' + entry.code,
      );
      for (const name of Object.keys({
        ...entry.files,
        ...entry.appendFiles,
      })) {
        await engine.lintFiles([join(folder, "src/shared/ds", name)]);
      }
      for (const file of entry.cleanFiles ?? [])
        await engine.lintFiles([join(folder, "src/shared/ds", file)]);
      const [result] = await engine.lintFiles([target]);
      assert.deepEqual(
        [...new Set(result.messages.map((message) => message.ruleId))].sort(),
        entry.expected.toSorted(),
        entry.name + JSON.stringify(result.messages),
      );
      const fullCases = [
        "generated token arbitrary prop",
        "generated token semantic prop",
        "generated token wrong category with flow",
        "nested forged Badge map",
        "nested forged Badge switch",
        "different prop cannot prove tone",
        "spread overwrites tone",
        "narrowed conditional only one branch",
        "foreign probe cannot prove component",
        "kind dynamic class",
        "color tokens cannot be variant",
      ];
      if (entry.line !== undefined)
        assert.deepEqual(
          result.messages.map((message) => message.line),
          [entry.line],
          entry.name,
        );
      if (entry.minimum !== undefined)
        assert.ok(result.messages.length >= entry.minimum, entry.name);
      for (const file of entry.cleanFiles ?? [])
        assert.deepEqual(
          (await engine.lintFiles([join(folder, "src/shared/ds", file)]))[0]
            .messages,
          [],
          entry.name + file,
        );
      if (fullCases.includes(entry.name) || entry.full !== undefined) {
        const [full] = await base.lintFiles([target]);
        if (entry.expected.length === 0)
          assert.deepEqual(
            full.messages,
            [],
            entry.name + JSON.stringify(full.messages),
          );
        else if (entry.full === "reject")
          assert.deepEqual(
            [...new Set(full.messages.map((message) => message.ruleId))].sort(),
            (entry.fullExpected ?? entry.expected).toSorted(),
            entry.name + JSON.stringify(full.messages),
          );
        else
          assert.ok(
            full.messages.some((m) =>
              ["ds/public-props", "ds/static-classes"].includes(m.ruleId),
            ),
            entry.name + JSON.stringify(full.messages),
          );
      }
      if (
        entry.expected.length === 0 ||
        entry.compile === "valid" ||
        fullCases.includes(entry.name) ||
        entry.full !== undefined
      ) {
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
      if (target !== source) {
        const original = join(desktopRoot, "src/shared/ds", entry.sourcePath);
        if (existsSync(original)) {
          writeFileSync(target, readFileSync(original));
          await engine.lintFiles([target]);
        } else rmSync(target);
      }
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
