import assert from "node:assert/strict";
import test from "node:test";
import {
  cpSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";

test("DS03 Base UI callbacks stay internal while public event and DOM forwarding is rejected", async () => {
  const folder = mkdtempSync("ds03-public-events-");
  try {
    mkdirSync(join(folder, "src/shared/ds"), { recursive: true });
    mkdirSync(join(folder, "src/shared/platform"), { recursive: true });
    cpSync(
      join(desktopRoot, "src/shared/lib"),
      join(folder, "src/shared/lib"),
      { recursive: true },
    );
    cpSync(
      join(desktopRoot, "src/shared/platform/input.ts"),
      join(folder, "src/shared/platform/input.ts"),
    );
    cpSync(
      join(desktopRoot, "src/shared/ds/tokens.ts"),
      join(folder, "src/shared/ds/tokens.ts"),
    );
    const source = join(folder, "src/shared/ds/TextField.tsx"),
      project = join(folder, "tsconfig.json");
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
    const cases = [
      [
        'import{Input}from"@base-ui/react/input";const inputEventValue=(_event:ChangeEvent<HTMLInputElement>):"ignored"=>"ignored";export const Probe=():ReactElement=><Input onChange={(event):void=>{inputEventValue(event);}}/>;',
        "ds/no-dom-values",
      ],
      [
        'import {Input}from"@base-ui/react/input";import{inputEventValue}from"../platform/input";import type{InputValue}from"../lib";export const Probe=(props:{readonly onChange:(value:InputValue)=>void}):ReactElement=><Input onChange={(event):void=>{const value=inputEventValue(event);if(value.kind==="ok")props.onChange(value.value);}}/>;',
        null,
      ],
      [
        'import{Button}from"@base-ui/react/button";export const Probe=(props:{readonly onChange:(event:MouseEvent<HTMLButtonElement>)=>void}):ReactElement=><Button onClick={(event):void=>{props.onChange(event);}}/>;',
        "ds/public-props",
      ],
      [
        'import{Button}from"@base-ui/react/button";export const Probe=(props:{readonly onPick:(node:HTMLElement)=>void}):ReactElement=><Button onClick={(event):void=>{props.onPick(event.currentTarget);}}/>;',
        "ds/public-props",
      ],
      [
        'import{Checkbox}from"@base-ui/react/checkbox";export const Probe=(props:{readonly onPick:(value:{readonly reason:"none"|"input-change"})=>void}):ReactElement=><Checkbox.Root onCheckedChange={(_next,details):void=>{props.onPick(details);}}/>;',
        "ds/public-props",
      ],
      [
        'import{Checkbox}from"@base-ui/react/checkbox";export const Probe=(props:{readonly onPick:(value:{readonly reason:"none"|"input-change"})=>void}):ReactElement=><Checkbox.Root onCheckedChange={(_next,details):void=>{const forwarded=details;const receive=props.onPick;receive(forwarded);}}/>;',
        "ds/public-props",
      ],
      [
        'import{Checkbox}from"@base-ui/react/checkbox";export const Probe=(props:{readonly onPick:(value:"none"|"input-change")=>void}):ReactElement=><Checkbox.Root onCheckedChange={(_next,details):void=>{props.onPick(details.reason);}}/>;',
        null,
      ],
      [
        'import{Checkbox}from"@base-ui/react/checkbox";export const Probe=(props:{readonly onPick:(value:{readonly reason:"none"|"input-change"})=>void}):ReactElement=>{const {onPick}=props;return <Checkbox.Root onCheckedChange={(_next,details):void=>{onPick(details);}}/>;};',
        "ds/public-props",
      ],
      [
        "export const Probe=(props:{readonly optional?:UiText}):ReactElement=><input aria-label={props.optional}/>;",
        null,
      ],
      [
        'export const Probe=(props:{readonly optional?:"plain"}):ReactElement=><input aria-label={props.optional}/>;',
        "ds/no-ui-attributes",
      ],
      [
        'export const Probe=(props:{readonly state:"on"|"off"}):ReactElement=><span className={props.state==="on"?"text-ink":"text-alert"}/>;',
        null,
      ],
      [
        "export const Probe=(props:{readonly paint:UiText}):ReactElement=><span className={props.paint}/>;",
        "ds/static-classes",
      ],
      [
        'export const Probe=(props:{readonly state:"on"|"off"}):ReactElement=><span className={props.state==="on"?"text-ink":"text-red-500"}/>;',
        "better-tailwindcss/no-restricted-classes",
      ],
    ];
    for (const [code, rule] of cases) {
      writeFileSync(
        source,
        'import type{ReactElement,MouseEvent,ChangeEvent}from"react";import type{UiText}from"../lib";' +
          code,
      );
      const config = JSON.parse(readFileSync(project, "utf8"));
      config.compilerOptions.noUnusedLocals = false;
      writeFileSync(project, JSON.stringify(config));
      const compiled = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", project],
        { encoding: "utf8" },
      );
      assert.equal(compiled.status, 0, compiled.stdout + compiled.stderr);
      const reports = (await engine.lintFiles([source]))[0].messages.filter(
        (m) => m.ruleId !== "@typescript-eslint/no-unused-vars",
      );
      if (rule === null) assert.deepEqual(reports, []);
      else
        assert.ok(
          reports.some((m) => m.ruleId === rule),
          JSON.stringify(reports),
        );
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});

test("DS03 DOM boundary export validation terminates for value promises and rejects nested DOM promises", async () => {
  const folder = mkdtempSync("ds03-export-values-");
  try {
    const source = join(folder, "src/shared/platform/input.ts"),
      project = join(folder, "tsconfig.json");
    mkdirSync(join(source, ".."), { recursive: true });
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        compilerOptions: { rootDir: "src" },
        include: ["src"],
      }),
    );
    const lint = new ESLint({
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
    for (const [code, rejected] of [
      [
        'export const readValue=():Promise<"value">=>Promise.resolve("value");',
        false,
      ],
      [
        "export const readNode=():Promise<Readonly<{node:HTMLElement}>>=>Promise.resolve({node:document.documentElement});",
        true,
      ],
      [
        'import type{SyntheticEvent}from"react";export const lastEvent=(e:Readonly<SyntheticEvent<never,never>>):Readonly<SyntheticEvent<never,never>>=>e;',
        true,
      ],
      [
        'import type{DOMAttributes}from"react";export const handlers=():Readonly<DOMAttributes<never>>=>({});',
        true,
      ],
      [
        'export declare const callable:(()=>"ok") & {readonly node:HTMLElement};',
        true,
      ],
    ]) {
      writeFileSync(source, code);
      const compile = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", project],
        { encoding: "utf8" },
      );
      assert.equal(compile.status, 0, compile.stdout + compile.stderr);
      const result = (await lint.lintFiles([source]))[0];
      assert.equal(
        result.messages.some(
          (message) => message.ruleId === "ds/no-dom-exports",
        ),
        rejected,
        JSON.stringify(result.messages),
      );
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
