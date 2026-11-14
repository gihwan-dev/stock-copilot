import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
const cases = [
  [
    "Button",
    '<Button size="control-height-sm">{label}</Button>',
    [
      '<Button size="icon-md">{label}</Button>',
      '<Button variant="unknown">{label}</Button>',
      '<Button variant="buy">{label}</Button>',
      '<Button variant="sell">{label}</Button>',
      "<Button disabled={true}>{label}</Button>",
      "<Button onClick={(e:MouseEvent):void=>{consume(e);}}>{label}</Button>",
    ],
  ],
  [
    "IconButton",
    '<IconButton icon="search" label={label} size="control-height"/>',
    [
      '<IconButton icon="search" label={label} size="icon-md"/>',
      '<IconButton icon="search" label="Search"/>',
      '<IconButton icon="search" label={label} pressed={true}/>',
      '<IconButton icon="search" label={label} expanded={false}/>',
      '<IconButton icon="search" label={label} tooltipSide="bottom"/>',
      '<IconButton icon="search" label={label} tooltip="hidden"/>',
    ],
  ],
  [
    "Text",
    '<Text variant="body-sm" tone="ink-muted" as="span">{label}</Text>',
    [
      '<Text variant="num-md">{label}</Text>',
      '<Text tone="default">{label}</Text>',
      '<Text as="button">{label}</Text>',
      "<Text>Search</Text>",
    ],
  ],
  [
    "Heading",
    "<Heading level={1}>{label}</Heading>",
    [
      "<Heading level={4}>{label}</Heading>",
      '<Heading level="1">{label}</Heading>',
      "<Heading>Search</Heading>",
      "<Heading id={label}>{label}</Heading>",
    ],
  ],
  [
    "TextField",
    '<TextField label={label} value={emptyInputValue} onChange={(value:InputValue):void=>{consume(value);}} type="secret"/>',
    [
      '<TextField label={label} value="input"/>',
      '<TextField label={label} placeholder="Input"/>',
      "<TextField label={label} disabled={true}/>",
      "<TextField label={label} onChange={(e:ChangeEvent<HTMLInputElement>):void=>{consume(e);}}/>",
      "<TextField label={label} onCommit={(value:InputValue):void=>{consume(value);}}/>",
      '<TextField label={label} type="password"/>',
    ],
  ],
  [
    "Select",
    "<Select label={label} options={options} value={emptyInputValue}/>",
    [
      '<Select label={label} options={options} value="raw"/>',
      '<Select label={label} options={[{value:emptyInputValue,label:"Raw"}]}/>',
      "<Select label={label} options={options} disabled={false}/>",
      "<Select label={label} options={options} onChange={(e:ChangeEvent<HTMLSelectElement>):void=>{consume(e);}}/>",
    ],
  ],
  [
    "Checkbox",
    '<Checkbox label={label} checked="checked" defaultChecked="unchecked"/>',
    [
      "<Checkbox label={label} checked={true}/>",
      "<Checkbox label={label} defaultChecked={false}/>",
      "<Checkbox label={label} onChange={(value:boolean):void=>{consume(value);}}/>",
    ],
  ],
  [
    "Switch",
    '<Switch label={label} checked="on" defaultChecked="off"/>',
    [
      "<Switch label={label} checked={true}/>",
      "<Switch label={label} defaultChecked={false}/>",
      "<Switch label={label} onChange={(value:boolean):void=>{consume(value);}}/>",
    ],
  ],
  [
    "Disclosure",
    '<Disclosure title={label} defaultOpen="open"><Text>{label}</Text></Disclosure>',
    [
      "<Disclosure title={label} open={true}><Text>{label}</Text></Disclosure>",
      "<Disclosure title={label} defaultOpen={false}><Text>{label}</Text></Disclosure>",
      "<Disclosure title={label} onOpenChange={(e:MouseEvent):void=>{consume(e);}}><Text>{label}</Text></Disclosure>",
      "<Disclosure title={label}>Raw</Disclosure>",
    ],
  ],
  [
    "SegmentedControl",
    "<SegmentedControl label={label} options={options} value={emptyInputValue}/>",
    [
      "<SegmentedControl label={label} options={[{value:emptyInputValue,label}]}/>",
      '<SegmentedControl label={label} options={options} value="raw"/>',
      "<SegmentedControl label={label} options={options} onChange={(e:MouseEvent):void=>{consume(e);}}/>",
      "<SegmentedControl label={label} options={[...options,...options,...options]}/>",
    ],
  ],
];
for (const [name, valid, invalid] of cases) {
  test(`DS03 ${name} public props accept design values and reject foreign tokens, booleans and events`, () => {
    const folder = mkdtempSync("ds03-component-types-");
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
        'import type {ReactElement,MouseEvent,ChangeEvent} from "react"; import {Button,IconButton,Text,Heading,TextField,Select,Checkbox,Switch,Disclosure,SegmentedControl} from "./index"; import {text,emptyInputValue} from "../lib";import type {InputValue}from "../lib"; const label=text("app.title","ko"); const options=[{value:emptyInputValue,label},{value:emptyInputValue,label}] as const; const consume=(value:unknown):void=>{JSON.stringify(value);};';
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
