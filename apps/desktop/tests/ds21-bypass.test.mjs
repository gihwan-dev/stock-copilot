import assert from "node:assert/strict";
import { test } from "node:test";
import { cpSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
test("DS21 existing lint rejects action assertion and spread bypasses", async () => {
  const folder = mkdtempSync("ds04-focus-");
  try {
    cpSync(join(desktopRoot, "src"), join(folder, "src"), { recursive: true });
    const project = join(folder, "tsconfig.json");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        include: ["src"],
        compilerOptions: { rootDir: "src" },
      }),
    );
    const engine = new ESLint({
      cwd: folder,
      overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
      overrideConfig: [
        {
          settings: { "boundaries/root-path": folder },
          languageOptions: {
            parserOptions: {
              projectService: false,
              project,
              tsconfigRootDir: folder,
            },
          },
        },
      ],
    });
    const reports = async (file) =>
      (await engine.lintFiles([file])).flatMap((result) => result.messages);
    const probe = join(folder, "src/app/Probe.tsx");
    const header =
      'import type {ReactElement} from "react";import {Card,Text} from "../shared/ds";import {text} from "../shared/lib";const label=text("app.title","ko");';
    for (const [body, rule] of [
      [
        "<Card actions={<Text>{label}</Text> as any}><Text>{label}</Text></Card>",
        "@typescript-eslint/no-explicit-any",
      ],
      [
        "<Card {...{actions:<Text>{label}</Text>}}><Text>{label}</Text></Card>",
        "guard/no-jsx-spread",
      ],
      [
        "<Card actions={{label,onPress:():void=>{return;}}}><Text>{label}</Text></Card>",
        null,
      ],
    ]) {
      writeFileSync(
        probe,
        header + "export const Probe=():ReactElement=>" + body + ";",
      );
      const found = await reports(probe);
      if (rule === null) assert.deepEqual(found, []);
      else
        assert.ok(
          found.some((message) => message.ruleId === rule),
          JSON.stringify(found),
        );
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
