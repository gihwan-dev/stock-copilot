import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";

const prefix =
  'import type {RefObject,Ref,MouseEvent,ComponentProps} from "react"; import type {SpacingToken,ColorToken,FontToken} from "./tokens"; type Text=string & {readonly text:unique symbol}; type Id=string & {readonly id:unique symbol}; ';
const component = (props) =>
  `export const Probe=(props:${props}):null=>props ? null : null;`;
const witnesses = JSON.parse(
  readFileSync(
    new URL(
      "../../../harness/eslint-violations/ds10-public.json",
      import.meta.url,
    ),
    "utf8",
  ),
);

test("DS10 public contracts reject bypasses with actual app types and cannot be disabled inline", async () => {
  const folder = mkdtempSync("ds10-public-");
  const source = join(folder, "src/shared/ds/Probe.tsx");
  const project = join(folder, "tsconfig.json");
  mkdirSync(join(source, ".."), { recursive: true });
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
  writeFileSync(
    join(folder, "src/shared/ds/tokens.ts"),
    readFileSync(join(desktopRoot, "src/shared/ds/tokens.ts")),
  );
  const engine = (level = "error", extraRules = {}) =>
    new ESLint({
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
          rules: { ...extraRules, "ds/public-props": level },
        },
      ],
    });
  const on = engine();
  const effective = await on.calculateConfigForFile(source);
  const isolatedRules = Object.fromEntries(
    Object.keys(effective.rules).map((rule) => [rule, "off"]),
  );
  const isolated = engine("error", isolatedRules),
    off = engine("off", isolatedRules);
  try {
    for (const [name, code] of witnesses) {
      writeFileSync(source, prefix + code);
      const compile = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc6"),
        ["-p", project],
        { encoding: "utf8" },
      );
      assert.equal(compile.status, 0, name + compile.stdout + compile.stderr);
      const errors = (await on.lintFiles([source]))[0].messages;
      assert.ok(
        (await isolated.lintFiles([source]))[0].messages.length > 0,
        name,
      );
      assert.ok(
        errors.some((m) => m.ruleId === "ds/public-props"),
        name + JSON.stringify(errors),
      );
      assert.deepEqual((await off.lintFiles([source]))[0].messages, [], name);
    }
    writeFileSync(
      source,
      prefix +
        "\n// eslint-disable-next-line ds/public-props -- witness\n" +
        component("Readonly<{className:Text}>"),
    );
    assert.ok(
      (await on.lintFiles([source]))[0].messages.some(
        (m) =>
          m.ruleId ===
          "@eslint-community/eslint-comments/no-restricted-disable",
      ),
    );
    writeFileSync(
      source,
      prefix +
        component(
          'Readonly<{label:Text; value:Text; decimal:Text; id:Id; gap:SpacingToken; color:ColorToken; ref?:RefObject<HTMLDivElement|null>; onChange:(value:Text)=>void; mode:"grid"|"list"}>',
        ),
    );
    assert.deepEqual(
      (await on.lintFiles([source]))[0].messages.filter(
        (m) => m.ruleId === "ds/public-props",
      ),
      [],
    );
    for (const props of [
      'Readonly<{children:import("react").ReactNode}>',
      "Readonly<{fontFamily:FontToken}>",
      "Readonly<{items:readonly Text[]; onChange:(value:Text)=>void}>",
      "Readonly<{columns:3|4|6|8|9|12}>",
      'Readonly<{gap:Extract<SpacingToken,"space-1"|"space-2">}>',
      "Readonly<{ref?:RefObject<HTMLDivElement|null>}>",
      "Readonly<{onSubmit:(v:Text)=>Promise<void>}>",
      "Readonly<{selected:ReadonlySet<Id>}>",
      "Readonly<{values:ReadonlyMap<Id,Text>}>",
      'Readonly<{icon:import("react").FC<{label:Text}>}>',
      'Readonly<{icon:import("react").ComponentType<{label:Text}>}>',
      "Readonly<{items:ReadonlyArray<Text>}>",
      "Readonly<{result:Promise<Text>}>",
      "Readonly<{result:Promise<readonly Text[]>}>",
      "Readonly<{groups:ReadonlyMap<Id,ReadonlySet<Text>>}>",
      "Readonly<{callback:()=>Promise<void>}>",
      "Readonly<{columns:{narrow:3|4|6;wide:12}}>",
      "Readonly<{span:3|4|6|12;level:1|2|3}>",
      "Readonly<{layout:{gap:SpacingToken;color:ColorToken}}>",
      "Readonly<{layouts:readonly {gap:SpacingToken}[]}>",
    ]) {
      writeFileSync(source, prefix + component(props));
      assert.deepEqual(
        (await isolated.lintFiles([source]))[0].messages,
        [],
        props,
      );
    }
    writeFileSync(
      source,
      prefix +
        "export const Probe=<T,>(p:{items:readonly T[]}):null=>p ? null:null;",
    );
    assert.deepEqual((await isolated.lintFiles([source]))[0].messages, []);
    writeFileSync(
      source,
      prefix +
        "const Grid=(p:{columns:{narrow:3|4|6;wide:12};span:3|4|6|12;level:1|2|3}):null=>p ? null:null; export const Probe=(p:{columns:{narrow:3|4|6;wide:12};span:3|4|6|12})=><Grid columns={p.columns} span={p.span} level={1}/>;",
    );
    assert.deepEqual((await isolated.lintFiles([source]))[0].messages, []);
    writeFileSync(
      source,
      prefix +
        'export const Probe=(props:Readonly<{paint:Text}>):import("react").ReactElement=><div className={props.paint}/>;',
    );
    assert.ok(
      (await on.lintFiles([source]))[0].messages.some(
        (m) => m.ruleId === "ds/static-classes",
      ),
    );
    writeFileSync(
      source,
      prefix +
        "const internal=(props:Readonly<{className:Text}>):null=>props ? null : null; export const Probe=internal as unknown as (props:Readonly<{label:Text}>)=>null;",
    );
    const erased = (await on.lintFiles([source]))[0].messages;
    assert.ok(
      erased.some(
        (m) => m.ruleId === "@typescript-eslint/consistent-type-assertions",
      ),
    );
    mkdirSync(join(folder, "src/shared/lib"), { recursive: true });
    for (const name of ["brand.ts", "result.ts"])
      writeFileSync(
        join(folder, "src/shared/lib", name),
        readFileSync(join(desktopRoot, "src/shared/lib", name)),
      );
    writeFileSync(
      join(folder, "src/shared/lib/index.ts"),
      'export type {UiText,EntityId} from "./brand";',
    );
    writeFileSync(
      source,
      'import type {ReactElement,RefObject} from "react"; import type {SpacingToken} from "./tokens"; import type {UiText,EntityId} from "../lib"; export const Probe=({label,value,decimal,id,gap,ref,onChange,mode}:{readonly label:UiText;readonly value:UiText;readonly decimal:UiText;readonly id:EntityId;readonly gap:SpacingToken;readonly ref:RefObject<HTMLDivElement|null>;readonly onChange:(value:UiText)=>void;readonly mode:"grid"|"list"}):ReactElement=>{const Tag:"span"|"em"|"s"="em";return <div id={id} ref={ref} data-gap={gap} data-mode={mode}><button type="button" onClick={()=>{onChange(value);}}>{label}</button><Tag lang="fr">{decimal}</Tag></div>;}',
    );
    assert.deepEqual((await on.lintFiles([source]))[0].messages, []);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});

test("DS10 generated token deletion and callback refs fail both TypeScript compilers", () => {
  const folder = mkdtempSync("ds10-types-");
  try {
    const project = join(folder, "tsconfig.json");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        compilerOptions: { rootDir: "." },
        include: ["*.tsx", "*.ts"],
      }),
    );
    const tokens = readFileSync(
      join(desktopRoot, "src/shared/ds/tokens.ts"),
      "utf8",
    );
    const file = join(folder, "Probe.tsx");
    const valid =
      'import type {RefObject} from "react"; import type {SpacingToken} from "./tokens"; export const Probe=(props:Readonly<{gap:SpacingToken;ref?:RefObject<HTMLDivElement|null>}>):null=>props ? null : null; export const example=<Probe gap="space-1"/>;';
    const compile = (binary) =>
      spawnSync(
        join(desktopRoot, "node_modules/.bin", binary),
        ["-p", project],
        { encoding: "utf8" },
      );
    for (const binary of ["tsc", "tsc6"]) {
      writeFileSync(join(folder, "tokens.ts"), tokens);
      writeFileSync(file, valid);
      const before = compile(binary);
      assert.equal(before.status, 0, binary + before.stdout + before.stderr);
      writeFileSync(
        join(folder, "tokens.ts"),
        tokens.replace('  | "space-1"\n', ""),
      );
      const removed = compile(binary);
      assert.notEqual(removed.status, 0);
      assert.match(removed.stdout, /2322|2820/);
      writeFileSync(join(folder, "tokens.ts"), tokens);
      writeFileSync(
        file,
        valid + 'export const bad=<Probe gap="space-1" ref={()=>undefined}/>;',
      );
      const ref = compile(binary);
      assert.notEqual(ref.status, 0);
      assert.match(ref.stdout, /2322|2820/);
      writeFileSync(
        file,
        valid +
          "export const Level=(p:{readonly level:1|2|3}):null=>p ? null:null;export const computed=<Level level={1+1}/>;",
      );
      const computed = compile(binary);
      assert.notEqual(computed.status, 0);
      assert.match(computed.stdout, /2322/);
      writeFileSync(
        file,
        valid +
          "export const Grid=(p:{readonly span:3|4|6|8|9|12}):null=>p ? null:null;export const invalid=<Grid span={5}/>;",
      );
      const invalid = compile(binary);
      assert.notEqual(invalid.status, 0);
      assert.match(invalid.stdout, /2322/);
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
