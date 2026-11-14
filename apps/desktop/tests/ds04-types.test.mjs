import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
const cases = [
  [
    "Badge",
    '<Badge tone="up" mono="mono">{label}</Badge>',
    [
      '<Badge tone="warning">{label}</Badge>',
      "<Badge mono={true}>{label}</Badge>",
      "<Badge>Raw</Badge>",
      '<Badge className="text-up">{label}</Badge>',
    ],
  ],
  [
    "StatusBar",
    '<StatusBar tone="error">{label}</StatusBar>',
    [
      '<StatusBar tone="up">{label}</StatusBar>',
      "<StatusBar>Raw</StatusBar>",
      "<StatusBar onClick={(e:MouseEvent):void=>{consume(e);}}/>",
    ],
  ],
  [
    "Card",
    "<Card title={label}><Text>{label}</Text></Card>",
    [
      '<Card className="flex"><Text>{label}</Text></Card>',
      '<Card title="Raw"><Text>{label}</Text></Card>',
    ],
  ],
  [
    "EmptyState",
    '<EmptyState title={label} icon="wallet"/>',
    ['<EmptyState title="Raw"/>', '<EmptyState title={label} icon="missing"/>'],
  ],
  [
    "Skeleton",
    '<Skeleton variant="block" height="chart-height-sm"/>',
    [
      '<Skeleton height="dialog-width"/>',
      "<Skeleton lines={true}/>",
      '<Skeleton variant="block" lines={3}/>',
    ],
  ],
  [
    "Table",
    '<Table.Root dense="dense"><Table.Body><Table.Row><Table.Cell align="end"><Text>{label}</Text></Table.Cell></Table.Row></Table.Body></Table.Root>',
    [
      "<Table.Root dense={true}><Table.Body/></Table.Root>",
      '<Table.Cell className="flex"/>',
      "<Table.Header onClick={(e:MouseEvent):void=>{consume(e);}}/>",
      '<Table.RowHeader style={{color:"red"}}/>',
    ],
  ],
  [
    "Dialog",
    '<Dialog open="closed" title={label} wide="wide" onClose={(reason):void=>{consume(reason);}}><Text>{label}</Text></Dialog>',
    [
      "<Dialog open={true} title={label}><Text>{label}</Text></Dialog>",
      '<Dialog open="closed" title="Raw"><Text>{label}</Text></Dialog>',
      '<Dialog open="open" title={label} onClose={(e:MouseEvent):void=>{consume(e);}}><Text>{label}</Text></Dialog>',
      '<Dialog open="closed" title={label} inline="inline"><Text>{label}</Text></Dialog>',
    ],
  ],
];
for (const [name, valid, invalid] of cases) {
  test(`DS04 ${name} public props accept design values and reject foreign tokens, booleans and events`, () => {
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
      writeFileSync(
        probe,
        header +
          "export const Probe=():ReactElement=><>" +
          invalid.join("") +
          "</>;",
      );
      const rejected = compile();
      assert.notEqual(rejected.status, 0);
      assert.equal(
        (rejected.stdout.match(/error TS/g) ?? []).length,
        invalid.length,
        rejected.stdout + rejected.stderr,
      );
    } finally {
      rmSync(folder, { recursive: true, force: true });
    }
  });
}
