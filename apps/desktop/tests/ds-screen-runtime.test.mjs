import assert from "node:assert/strict";
import test from "node:test";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  rmSync,
  readFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { ESLint } from "eslint";
import { desktopRoot, repoRoot } from "./paths.mjs";

const cases = JSON.parse(
  readFileSync(join(repoRoot, "harness/ds-style/screen-cases.json"), "utf8"),
);
test("DS09 bypasses compile then fail the effective application lint", async () => {
  const folder = mkdtempSync(join(desktopRoot, ".ds-screen-"));
  const config = join(folder, "tsconfig.json");
  const source = join(folder, "src/features/probe/ui/Probe.tsx");
  for (const path of [
    "src/features/probe/ui",
    "src/shared/ds",
    "src/shared/lib",
  ])
    mkdirSync(join(folder, path), { recursive: true });
  writeFileSync(
    config,
    JSON.stringify({
      extends: join(desktopRoot, "tsconfig.json"),
      compilerOptions: { rootDir: "src" },
      include: ["src"],
    }),
  );
  writeFileSync(
    join(folder, "src/shared/ds/index.ts"),
    'import type {ReactElement,RefObject,MouseEvent} from "react"; export const Button=(props:Readonly<{title?: string; placeholder?:string; paint?:string; className?:string; style?:object; children?:ReactElement; render?:()=>ReactElement; ref?:RefObject<HTMLDivElement|null>|(()=>undefined); onClick?:(event:MouseEvent<HTMLButtonElement>)=>void}>):null=>props ? null : null;',
  );
  writeFileSync(
    join(folder, "src/shared/lib/index.ts"),
    'import type {z} from "zod"; export type UiText=string & Readonly<z.core.$brand<"UiText">>; export const imported="bg-primary";',
  );
  writeFileSync(
    join(folder, "src/shared/ds/internal.ts"),
    'export {Button} from "./index";',
  );
  const linter = new ESLint({
    cwd: folder,
    overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
    overrideConfig: [
      {
        languageOptions: {
          parserOptions: {
            projectService: false,
            project: config,
            tsconfigRootDir: folder,
          },
        },
        settings: { "boundaries/root-path": folder },
      },
    ],
  });
  try {
    for (const [name, spec] of Object.entries(cases)) {
      writeFileSync(source, spec.code);
      const run = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", config],
        { encoding: "utf8" },
      );
      assert.equal(run.status, 0, name + run.stdout + run.stderr);
      const messages = (await linter.lintFiles([source]))[0].messages;
      assert.ok(
        messages.some((m) => m.ruleId === spec.rule),
        name + JSON.stringify(messages),
      );
      process.stdout.write(`DS09 ${name}: tsc PASS, ${spec.rule}\n`);
    }
    writeFileSync(
      source,
      'import type {ReactElement} from "react"; import {Button} from "../../../shared/ds"; export const Probe=():ReactElement=><Button/>;',
    );
    assert.deepEqual((await linter.lintFiles([source]))[0].messages, []);
    writeFileSync(
      source,
      'import type {ReactElement} from "react"; import type {UiText} from "../../../shared/lib"; import {Button} from "../../../shared/ds"; export const Probe=({title}:{readonly title:UiText}):ReactElement=><Button title={title}/>;',
    );
    const validRun = spawnSync(
      join(desktopRoot, "node_modules/.bin/tsc"),
      ["-p", config],
      { encoding: "utf8" },
    );
    assert.equal(validRun.status, 0, validRun.stdout + validRun.stderr);
    assert.deepEqual((await linter.lintFiles([source]))[0].messages, []);
    const messages = join(folder, "src/features/probe/messages.ts");
    writeFileSync(
      messages,
      'import type {UiText} from "../../shared/lib"; const catalog={paint:{ko:("bg"+"-primary") as UiText}} as const; export const text=(key:"paint",locale:"ko"):UiText=>catalog[key][locale];',
    );
    writeFileSync(
      source,
      'import type {ReactElement} from "react"; import {Button} from "../../../shared/ds"; import {text} from "../messages"; export const Probe=():ReactElement=><Button paint={text("paint","ko")}/>;',
    );
    const gapRun = spawnSync(
      join(desktopRoot, "node_modules/.bin/tsc"),
      ["-p", config],
      { encoding: "utf8" },
    );
    assert.equal(gapRun.status, 0, gapRun.stdout + gapRun.stderr);
    for (const result of await linter.lintFiles([source, messages]))
      assert.deepEqual(result.messages, []);
    process.stdout.write(
      "DS09 KNOWN GAP: composed UiText through arbitrary paint prop passes tsc and screen/messages lint; DS10 contract required\n",
    );
    for (const path of [
      "pages/Probe.tsx",
      "widgets/Probe.tsx",
      "components/Probe.tsx",
    ]) {
      const file = join(folder, "src", path);
      mkdirSync(join(file, ".."), { recursive: true });
      writeFileSync(file, "export const Probe=():null=>null;");
      assert.ok(
        (await linter.lintFiles([file]))[0].messages.some(
          (m) => m.ruleId === "boundaries/no-unknown-files",
        ),
        path,
      );
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
