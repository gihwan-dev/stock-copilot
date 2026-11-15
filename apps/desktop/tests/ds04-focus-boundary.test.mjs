import assert from "node:assert/strict";
import { test } from "node:test";
import { cpSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
test("DS04 only the real input boundary consumes dialog refs; direct and counterfeit DOM routes stay forbidden", async () => {
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
    const dialog = join(folder, "src/shared/ds/Dialog.tsx");
    assert.deepEqual(await reports(dialog), []);
    writeFileSync(
      dialog,
      'import {useRef,type ReactElement} from "react";export const Dialog=():ReactElement=>{const popupRef=useRef<HTMLDivElement>(null);return <div ref={popupRef} onClick={():void=>{popupRef.current?.focus();}}/>;};',
    );
    assert.ok(
      (await reports(dialog)).some(
        (message) => message.ruleId === "ds/no-dom-values",
      ),
    );
    writeFileSync(
      dialog,
      'import {useRef,type ReactElement,type RefObject} from "react";const focusDialogInput=(ref:Readonly<RefObject<HTMLDivElement|null>>):void=>{ref.current?.focus();};export const Dialog=():ReactElement=>{const popupRef=useRef<HTMLDivElement>(null);return <div ref={popupRef} onClick={():void=>{focusDialogInput(popupRef);}}/>;};',
    );
    assert.ok(
      (await reports(dialog)).some(
        (message) => message.ruleId === "ds/no-dom-values",
      ),
    );
    const other = join(folder, "src/shared/ds/Probe.tsx");
    writeFileSync(
      other,
      'import {focusDialogInput} from "../platform/input";import {useRef,type ReactElement} from "react";export const Probe=():ReactElement=>{const popupRef=useRef<HTMLDivElement>(null);return <div ref={popupRef} onClick={():void=>{focusDialogInput(popupRef);}}/>;};',
    );
    assert.ok(
      (await reports(other)).some(
        (message) => message.ruleId === "boundaries/dependencies",
      ),
    );
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
