import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { ESLint } from "eslint";
import { dsConfigs, DS_RULES } from "../eslint/ds-config.mjs";
import * as lists from "../eslint/ds-allowlists.mjs";
import { desktopRoot } from "./paths.mjs";
const render = (jsx) =>
  'import type {ReactElement} from "react";\nexport const Probe = (): ReactElement => ' +
  jsx +
  ";";
const cases = [
  "export const handleNarrow = (event:{readonly currentTarget:{readonly focus:()=>void}|null}):void=>event.currentTarget?.focus();",
  'import {useRef} from "react"; import type {ReactElement} from "react"; export const Probe=():ReactElement=>{const originalRef=useRef<HTMLDivElement>(null);const ref:{readonly current:{readonly focus:()=>void}|null}=originalRef;const key="current" as const;const {[key]:element}=ref;return <button type={`button`} onClick={()=>element?.focus()}/>;};',
  'export const mutate = (el: Readonly<HTMLDivElement>):void=>el.setHTMLUnsafe("<style/>");',
  "export const handle = (event: Readonly<Event>):void=>event.stopPropagation();",
  "export const Parser = DOMParser;",
  render("<div ref={()=>new MutationObserver(()=>undefined).disconnect()}/>"),
  render("<div ref={()=>new ResizeObserver(()=>undefined).disconnect()}/>"),
  render("<div ref={()=>new Range().detach()}/>"),
  render('<div ref={el=>el?.setHTMLUnsafe("<style>*{color:red}</style>")}/>'),
  render(
    '<div ref={el=>el?.ownerDocument.createRange().createContextualFragment("<style/>") ? undefined : undefined}/>',
  ),
  render(
    '<div ref={()=>new DOMParser().parseFromString("<style/>","text/html") ? undefined : undefined}/>',
  ),
  render(
    '<div ref={el=>el?.attachShadow({mode:"open"}) ? undefined : undefined}/>',
  ),
  render(
    '<div ref={el=>el?.ownerDocument.createElementNS("http://www.w3.org/1999/xhtml","style") ? undefined : undefined}/>',
  ),
  render(
    "<div ref={el=>el?.ownerDocument.styleSheets.item(0)?.deleteRule(0)}/>",
  ),
  render('<div ref={()=>new CSSStyleSheet().replaceSync("body{}")}/>'),
  render(
    "<div ref={el=>new Animation(new KeyframeEffect(el,{opacity:[1,.3]},200)).cancel()}/>",
  ),
  render(
    '<div ref={el=>el?.ownerDocument.documentElement.removeAttribute("data-theme")}/>',
  ),
  render('<div ref={el=>el?.part.add("theme")}/>'),
  'import type {RefObject} from "react";\nexport const inspect = (ref: Readonly<RefObject<HTMLDivElement|null>>):void => ref.current?.focus();',
  render(
    "<button type={`button`} onClick={event=>event.currentTarget.focus()}/>",
  ),
  render(
    '<button type={`button`} onClick={event=>event.target.addEventListener("x",()=>undefined)}/>',
  ),
  render(
    "<button type={`button`} onClick={event=>event.nativeEvent.stopImmediatePropagation()}/>",
  ),
];
test("DS02 final DOM routes fail real lint after native typecheck and approved icon and data cases pass", async () => {
  const folder = mkdtempSync(join(desktopRoot, ".ds-final-"));
  const source = join(folder, "src/shared/ds/Probe.tsx");
  mkdirSync(join(folder, "src/shared/ds"), { recursive: true });
  mkdirSync(join(folder, "src/shared/lib"), { recursive: true });
  const configPath = join(folder, "tsconfig.json");
  writeFileSync(
    configPath,
    JSON.stringify({
      extends: join(desktopRoot, "tsconfig.json"),
      compilerOptions: { rootDir: "src" },
      include: ["src"],
    }),
  );
  const make = (extra = []) =>
    new ESLint({
      cwd: folder,
      overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
      overrideConfig: [
        ...extra,
        {
          languageOptions: {
            parserOptions: {
              projectService: false,
              project: configPath,
              tsconfigRootDir: folder,
            },
          },
          settings: { "boundaries/root-path": folder },
        },
      ],
    });
  const compile = () =>
    spawnSync(join(desktopRoot, "node_modules/.bin/tsc"), ["-p", configPath], {
      encoding: "utf8",
    });
  try {
    const lint = make();
    for (const code of cases) {
      writeFileSync(source, code);
      const run = compile();
      assert.equal(run.status, 0, run.stdout + run.stderr);
      const [result] = await lint.lintFiles([source]);
      assert.ok(
        result.messages.some((m) => m.ruleId === "ds/no-dom-values"),
        code + JSON.stringify(result.messages),
      );
    }
    const valid = [
      'export const readTarget=(value:{readonly target:"item"}):"item"=>value.target;',
      render('<a href="#add" lang="fr"/>'),
      render('<time dateTime="PT1S"/>'),
      'import type {ReactElement} from "react";\nexport const Probe=():ReactElement=>{ const Tag:"span"|"em"|"s"="em";return <Tag lang="fr"/>;};',
      'import {useRef} from "react";\nimport type {ReactElement} from "react";\nexport const Probe=():ReactElement=>{const ref=useRef<HTMLDivElement>(null);return <div ref={ref}/>;};',
    ];
    for (const code of valid) {
      writeFileSync(source, code);
      const run = compile();
      assert.equal(run.status, 0, run.stdout + run.stderr);
      assert.deepEqual((await lint.lintFiles([source]))[0].messages, [], code);
    }
    writeFileSync(
      source,
      render(
        '<svg className="fill-none stroke-current stroke-icon stroke-linecap-round stroke-linejoin-round" viewBox="0 0 24 24"><rect x={3} y={3} width={18} height={18} rx={2}/><circle cx={12} cy={12} r={4}/></svg>',
      ),
    );
    const icons = make(
      dsConfigs(DS_RULES, {
        ...lists,
        ICON_GEOMETRY_FILES: ["src/shared/ds/Probe.tsx"],
      }),
    );
    const run = compile();
    assert.equal(run.status, 0, run.stdout + run.stderr);
    assert.deepEqual((await icons.lintFiles([source]))[0].messages, []);
    rmSync(source);
    const format = join(folder, "src/shared/lib/format.ts");
    writeFileSync(
      format,
      'export const unit=():string=>new Intl.NumberFormat("fr",{style:"unit",unit:"meter"}).format(1);',
    );
    assert.equal(compile().status, 0);
    assert.deepEqual((await lint.lintFiles([format]))[0].messages, []);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
