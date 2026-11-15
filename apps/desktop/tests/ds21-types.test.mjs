import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
const cases = [
  [
    "Card",
    "<Card actions={{label,onPress:():void=>{consume(label);}}}><Text>{label}</Text></Card>",
    [
      "<Card actions={<Text>{label}</Text>}><Text>{label}</Text></Card>",
      "<Card actions={[{label,onPress:():void=>{}}]}><Text>{label}</Text></Card>",
      '<Card actions={{label,onPress:():void=>{},variant:"primary"}}><Text>{label}</Text></Card>',
    ],
  ],
  [
    "EmptyState",
    '<EmptyState title={label} action={{label,onPress:():void=>{consume(label);},disabled:"disabled"}}/>',
    [
      "<EmptyState title={label} action={<Text>{label}</Text>}/>",
      '<EmptyState title={label} action={{label:"raw",onPress:():void=>{}}}/>',
      "<EmptyState title={label} action={{label,onPress:():void=>{},disabled:true}}/>",
    ],
  ],
  [
    "Dialog",
    '<Dialog open="open" title={label} actions={{confirm:{label,onPress:():void=>{consume(label);}},cancel:{label,onPress:():void=>{consume(label);}}}}><Text>{label}</Text></Dialog>',
    [
      '<Dialog open="open" title={label} actions={<Text>{label}</Text>}><Text>{label}</Text></Dialog>',
      '<Dialog open="open" title={label} actions={[{label,onPress:():void=>{}}]}><Text>{label}</Text></Dialog>',
      '<Dialog open="open" title={label} actions={{other:{label,onPress:():void=>{}}}}><Text>{label}</Text></Dialog>',
      '<Dialog open="open" title={label} actions={{confirm:<Text>{label}</Text>}}><Text>{label}</Text></Dialog>',
    ],
  ],
];
for (const [name, valid, invalid] of cases) {
  test(`DS21 ${name} public props accept design values and reject foreign tokens, booleans and events`, () => {
    const folder = mkdtempSync("ds04-component-types-");
    try {
      cpSync(join(desktopRoot, "src"), join(folder, "src"), {
        recursive: true,
      });
      const project = join(folder, "tsconfig.json"),
        probe = join(folder, "src/shared/ds/TypeProbe.tsx");
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
      const header =
        'import type {ReactElement,MouseEvent,ChangeEvent} from "react"; import {Card,Table,Dialog,EmptyState,Skeleton,Text,Badge,StatusBar} from "./index"; import {text,emptyInputValue} from "../lib";import type {InputValue}from "../lib"; const label=text("app.title","ko"); const options=[{value:emptyInputValue,label},{value:emptyInputValue,label}] as const; const consume=(value:unknown):void=>{JSON.stringify(value);};';
      const compile = () =>
        spawnSync(join(desktopRoot, "node_modules/.bin/tsc"), ["-p", project], {
          encoding: "utf8",
        });
      writeFileSync(
        probe,
        header + "export const Probe=():ReactElement=><>" + valid + "</>;",
      );
      const accepted = compile();
      assert.equal(accepted.status, 0, accepted.stdout + accepted.stderr);
      for (const negative of invalid) {
        writeFileSync(
          probe,
          header + "export const Probe=():ReactElement=><>" + negative + "</>;",
        );
        const rejected = compile();
        assert.notEqual(
          rejected.status,
          0,
          negative + rejected.stdout + rejected.stderr,
        );
        assert.match(rejected.stdout, /TypeProbe\.tsx\(1,/u, negative);
      }
    } finally {
      rmSync(folder, { recursive: true, force: true });
    }
  });
}
