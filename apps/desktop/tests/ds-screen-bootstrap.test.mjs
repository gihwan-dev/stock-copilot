import assert from "node:assert/strict";
import test from "node:test";
import {
  cpSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { ESLint } from "./ds-runtime-environment.mjs";
import ts from "typescript";
import { desktopRoot } from "./paths.mjs";
import { SCREEN_DOM_BOOTSTRAP } from "../eslint/ds-screen-allowlists.mjs";
import { DOM_ACCESS_FILES } from "../eslint/ds-allowlists.mjs";
import { SCREEN_EXTERNAL_PACKAGES } from "../eslint/boundaries.mjs";

test("DS09 review main cannot calculate DOM values and platform exports cannot return DOM", async () => {
  const folder = mkdtempSync(join(desktopRoot, ".ds-bootstrap-"));
  for (const path of [
    "src/app",
    "src/shared/platform",
    "src/shared/ds",
    "src/shared/lib",
    "node_modules/review-motion",
  ])
    mkdirSync(join(folder, path), { recursive: true });
  const config = join(folder, "tsconfig.json");
  writeFileSync(
    config,
    JSON.stringify({
      extends: join(desktopRoot, "tsconfig.json"),
      compilerOptions: { rootDir: "src" },
      include: ["src"],
    }),
  );
  const main = join(folder, "src/app/main.tsx");
  const platform = join(folder, "src/shared/platform/index.ts");
  const make = (rules = {}) =>
    new ESLint({
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
          rules,
        },
      ],
    });
  const compile = () => {
    const run = spawnSync(
      join(desktopRoot, "node_modules/.bin/tsc"),
      ["-p", config],
      { encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stdout + run.stderr);
  };
  try {
    assert.deepEqual(SCREEN_DOM_BOOTSTRAP, []);
    cpSync(
      join(desktopRoot, "src/shared/lib"),
      join(folder, "src/shared/lib"),
      { recursive: true },
    );
    for (const path of [
      "app/main.tsx",
      "app/App.tsx",
      "shared/platform/index.ts",
    ])
      writeFileSync(
        join(folder, "src", path),
        readFileSync(join(desktopRoot, "src", path), "utf8"),
      );
    compile();
    for (const result of await make().lintFiles([main, platform]))
      assert.deepEqual(result.messages, []);
    const program = ts.createProgram([platform], {
      noEmit: true,
      strict: true,
    });
    const checker = program.getTypeChecker();
    const symbol = checker.getSymbolAtLocation(program.getSourceFile(platform));
    assert.deepEqual(
      checker.getExportsOfModule(symbol).map((s) => s.name),
      ["mountApp", "inputEventValue", "inputCommit", "pasteInputValue"],
    );
    const exported = checker.getExportsOfModule(symbol)[0];
    assert.deepEqual(
      checker
        .getTypeOfSymbolAtLocation(exported, program.getSourceFile(platform))
        .getCallSignatures()
        .map((signature) => signature.getReturnType().flags),
      [ts.TypeFlags.Void],
    );
    assert.deepEqual(DOM_ACCESS_FILES, ["src/shared/platform/index.ts"]);
    writeFileSync(
      join(folder, "src/shared/lib/index.ts"),
      'import type {z} from "zod"; export type UiText=string & Readonly<z.core.$brand<"UiText">>; export {parseInputValue,parseEntityId} from "./brand"; export type {InputValue,BrandError} from "./brand"; export {err} from "./result"; export type {Result} from "./result";',
    );
    const ds = join(folder, "src/shared/ds/Probe.tsx");
    writeFileSync(
      ds,
      'import type {ReactElement} from "react"; import type {UiText} from "../lib"; export const Probe=({label}:{readonly label:UiText}):ReactElement=><button type="button" aria-label={label} title={label} aria-valuetext={label}/>;',
    );
    compile();
    assert.deepEqual((await make().lintFiles([ds]))[0].messages, []);
    writeFileSync(
      ds,
      'import type {ReactElement} from "react"; export const Probe=({kind}:{readonly kind:"high"|"low"}):ReactElement=><button type="button" aria-label="Close dialog" title={kind} aria-valuetext="High"/>;',
    );
    compile();
    assert.equal(
      (await make().lintFiles([ds]))[0].messages.filter(
        (m) => m.ruleId === "ds/no-ui-attributes",
      ).length,
      3,
    );
    assert.deepEqual(
      (await make({ "ds/no-ui-attributes": "off" }).lintFiles([ds]))[0]
        .messages,
      [],
    );
    writeFileSync(
      ds,
      'import type {ReactElement} from "react"; import type {UiText} from "../lib"; export const Probe=({paint}:{readonly paint:UiText}):ReactElement=><button type="button" className={paint}/>;',
    );
    compile();
    assert.ok(
      (await make().lintFiles([ds]))[0].messages.some(
        (message) => message.ruleId === "ds/static-classes",
      ),
    );
    writeFileSync(
      main,
      'import {useRef} from "react"; export const Measured=():null=>{const ref=useRef<HTMLDivElement>(null);const width=ref.current?.ownerDocument.body.clientWidth;return width ? null : null;};',
    );
    writeFileSync(
      join(folder, "src/app/App.tsx"),
      'import {Measured} from "./main"; export const App=():null=>Measured();',
    );
    compile();
    assert.ok(
      (await make().lintFiles([main]))[0].messages.some(
        (m) => m.ruleId === "ds/no-dom-values",
      ),
    );
    assert.ok(
      !(
        await make({ "ds/no-dom-values": "off" }).lintFiles([main])
      )[0].messages.some((m) => m.ruleId === "ds/no-dom-values"),
    );
    writeFileSync(platform, "export const leak=():HTMLElement=>document.body;");
    compile();
    assert.ok(
      (await make().lintFiles([platform]))[0].messages.some(
        (m) => m.ruleId === "ds/no-dom-exports",
      ),
    );
    assert.deepEqual(
      (await make({ "ds/no-dom-exports": "off" }).lintFiles([platform]))[0]
        .messages,
      [],
    );
    writeFileSync(
      join(folder, "node_modules/review-motion/package.json"),
      JSON.stringify({ name: "review-motion", types: "index.d.ts" }),
    );
    writeFileSync(
      join(folder, "node_modules/review-motion/index.d.ts"),
      "export const motion:{readonly div:()=>null};",
    );
    writeFileSync(
      join(folder, "src/app/App.tsx"),
      "export const App=():null=>null;",
    );
    writeFileSync(
      main,
      'import {motion} from "review-motion"; import type {ReactElement} from "react"; export const Probe=():ReactElement=><motion.div/>;',
    );
    compile();
    const messages = (await make().lintFiles([main]))[0].messages;
    for (const rule of ["boundaries/dependencies", "ds/no-screen-elements"])
      assert.ok(
        messages.some((m) => m.ruleId === rule),
        rule,
      );
    assert.deepEqual(SCREEN_EXTERNAL_PACKAGES, [
      "react",
      "@tanstack/react-router",
      "@tanstack/react-query",
      "react-error-boundary",
    ]);
    writeFileSync(main, 'export {motion} from "review-motion";');
    compile();
    assert.ok(
      (await make().lintFiles([main]))[0].messages.some(
        (m) => m.ruleId === "boundaries/dependencies",
      ),
    );
    assert.deepEqual(
      (await make({ "boundaries/dependencies": "off" }).lintFiles([main]))[0]
        .messages,
      [],
    );
    writeFileSync(
      main,
      'import {motion} from "review-motion"; import type {ReactElement} from "react"; export const Probe=():ReactElement=><motion.div/>;',
    );
    const disabled = (
      await make({
        "boundaries/dependencies": "off",
        "ds/no-screen-elements": "off",
      }).lintFiles([main])
    )[0].messages;
    assert.deepEqual(disabled, []);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
