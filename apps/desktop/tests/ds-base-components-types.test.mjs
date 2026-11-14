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
import { mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";

test("DS03 Icon accepts only its generated size category and branded labels", () => {
  const folder = mkdtempSync("ds03-types-");
  try {
    cpSync(join(desktopRoot, "src"), join(folder, "src"), { recursive: true });
    mkdirSync(join(folder, "src/shared/ds"), { recursive: true });
    const project = join(folder, "tsconfig.json");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        compilerOptions: { rootDir: "src" },
        include: ["src"],
      }),
    );
    const probe = join(folder, "src/shared/ds/Probe.tsx");
    const header =
      'import { Icon } from "./Icon"; import { text } from "../lib";';
    const compile = () =>
      spawnSync(join(desktopRoot, "node_modules/.bin/tsc"), ["-p", project], {
        encoding: "utf8",
      });
    writeFileSync(
      probe,
      header +
        'export const Probe=():import("react").ReactElement=><Icon name="search" size="icon-sm" label={text("app.title","ko")}/>;',
    );
    const valid = compile();
    assert.equal(valid.status, 0, valid.stdout + valid.stderr);
    writeFileSync(
      probe,
      header +
        'export const Probe=():import("react").ReactElement=><><Icon name="search" size="control-height"/><Icon name="search" size="space-1"/><Icon name="search" size="text-sm"/><Icon name="search" size="sm"/><Icon name="search" label="Search"/><Icon name="unknown"/><Icon name="search" className="bg-primary"/><Icon name="search" style={{color:"red"}}/><Icon name="search" ref={():void=>undefined}/><Icon name="search" size="icon-lg" label={text("app.title","ko")}/></>;',
    );
    const invalid = compile();
    assert.notEqual(invalid.status, 0);
    assert.equal(
      (invalid.stdout.match(/error TS/g) ?? []).length,
      9,
      invalid.stdout + invalid.stderr,
    );
    writeFileSync(
      probe,
      header +
        'export const Probe=():import("react").ReactElement=><Icon name="search" size="icon-sm" label={text("app.title","ko")}/>;',
    );
    const tokens = join(folder, "src/shared/ds/tokens.ts");
    writeFileSync(
      tokens,
      readFileSync(tokens, "utf8").replace('  | "icon-lg"\n', ""),
    );
    const removed = compile();
    assert.notEqual(removed.status, 0);
    assert.match(removed.stdout, /Icon\.tsx.*TS2367/u);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
