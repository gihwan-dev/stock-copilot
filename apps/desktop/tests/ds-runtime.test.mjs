import { spawnSync } from "node:child_process";
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { ESLint } from "eslint";
import ts from "typescript";
import { desktopRoot } from "./paths.mjs";

const header = 'import type { ReactElement } from "react";\n';
const exported = (jsx) =>
  `${header}export const Probe = (): ReactElement => (${jsx});\n`;
const dom = [
  ['el?.style.setProperty("width", "50%")', "ds/no-dom-styling"],
  ['el?.classList.add("z-50")', "ds/no-dom-styling"],
  ['el?.setAttribute("class", "z-50")', "ds/no-dom-styling"],
  ['el?.setAttribute("style", "padding:1rem")', "ds/no-dom-styling"],
  ['el?.setAttributeNS(null, "class", "z-50")', "ds/no-dom-styling"],
  [
    'el?.toggleAttribute("hidden") === undefined ? undefined : undefined',
    "ds/no-dom-styling",
  ],
  ["el?.animate([{ opacity: 0 }], 100).cancel()", "ds/no-dom-styling"],
  [
    'el?.insertAdjacentHTML("beforeend", "<style>div{color:red}</style>")',
    "ds/no-dom-styling",
  ],
  ["el?.getRootNode().appendChild(new CSSStyleSheet())", "ds/no-dom-styling"],
];
const presentation = [
  ['fill="red"', "svg"],
  ['stroke="rebeccapurple"', "svg"],
  ['fill="transparent"', "svg"],
  ['fill="currentColor"', "svg"],
  ['fill="color-mix(in srgb, red, blue)"', "svg"],
  ['fill="lab(50% 0 0)"', "svg"],
  ['fill="hwb(0 0% 0%)"', "svg"],
  ['fill="light-dark(red, blue)"', "svg"],
  ['width="50%"', "svg"],
  ['width="0.8rem"', "svg"],
  ['width="1em"', "svg"],
  ['width="100vh"', "svg"],
  ['width="calc(100% - 1px)"', "svg"],
  ['width="13pt"', "svg"],
  ["width={13}", "svg"],
  ["size={13}", "input"],
  ['width={"1"+"3"+"px"}', "svg"],
  ['width={String(13)+"px"}', "svg"],
  ["width={`${13}px`}", "svg"],
];
const runtimeCases = [
  ...dom
    .filter(([code]) => !code.includes("appendChild"))
    .map(([code, rule]) => ({
      code: exported(`<div ref={(el) => ${code}} />`),
      rule,
    })),
  ...presentation.map(([attr, tag]) => ({
    code: exported(`<${tag} ${attr} />`),
    rule: "ds/no-presentation-props",
  })),
  {
    code:
      'import { createElement } from "react";\n' +
      header +
      'export const Probe = (): ReactElement => createElement("div", {className:"z-50 bg-red-500 p-(--x) dark:bg-primary"});',
    rule: "ds/no-element-factory",
  },
  {
    code:
      'import { createElement } from "react";\n' +
      header +
      'export const Probe = (): ReactElement => createElement("div", {[`className`]: "z-50"});',
    rule: "ds/no-style-object",
  },
  {
    code:
      'import { createElement } from "react";\n' +
      header +
      'export const Probe = (): ReactElement => createElement("div", {["class"+"Name"]: "z-50"});',
    rule: "ds/no-style-object",
  },
  {
    code:
      'import { createElement } from "react";\n' +
      header +
      'export const Probe = (): ReactElement => createElement("div", Object.fromEntries([["className", "z-50"]]));',
    rule: "ds/no-style-object",
  },
];

test("DS02 review runtime witnesses typecheck with actual app options and fail the actual ESLint configuration", async () => {
  const folder = mkdtempSync(join(desktopRoot, ".ds-runtime-"));
  const path = join(folder, "src/shared/ds/Probe.tsx");
  mkdirSync(join(folder, "src/shared/ds"), { recursive: true });
  const configPath = join(folder, "tsconfig.json");
  writeFileSync(
    configPath,
    JSON.stringify({
      extends: join(desktopRoot, "tsconfig.json"),
      compilerOptions: { rootDir: "src" },
      include: ["src"],
    }),
  );
  const config = ts.readConfigFile(configPath, ts.sys.readFile);
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, folder);
  const lint = new ESLint({
    cwd: folder,
    overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
    overrideConfig: [
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
  try {
    for (const { code, rule } of runtimeCases) {
      writeFileSync(path, code);
      const native = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", configPath],
        { encoding: "utf8" },
      );
      assert.equal(
        native.status,
        0,
        `${code}\n${native.stdout}${native.stderr}`,
      );
      const program = ts.createProgram([path], parsed.options);
      const diagnostics = ts.getPreEmitDiagnostics(program);
      assert.deepEqual(
        diagnostics.map((d) =>
          ts.flattenDiagnosticMessageText(d.messageText, "\n"),
        ),
        [],
        code,
      );
      const [result] = await lint.lintFiles([path]);
      assert.ok(
        result.messages.some((m) => m.ruleId === rule),
        `${rule}: ${code}\n${JSON.stringify(result.messages)}`,
      );
    }
    const lib = join(folder, "src/shared/lib");
    mkdirSync(lib, { recursive: true });
    writeFileSync(
      join(lib, "Box.tsx"),
      exported(
        '<div className="z-50 bg-red-500 p-(--x)" style={{padding:"1rem",color:"red"}} />',
      ),
    );
    writeFileSync(
      join(lib, "index.ts"),
      'export { Probe as Box } from "./Box";',
    );
    writeFileSync(
      path,
      'import { Box } from "../lib/index";\n' + exported("<Box />"),
    );
    assert.equal(
      spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", configPath],
        { encoding: "utf8" },
      ).status,
      0,
    );
    const results = await lint.lintFiles([path, join(lib, "Box.tsx")]);
    assert.ok(
      results.some((r) =>
        r.messages.some((m) => m.ruleId === "ds/no-shared-jsx"),
      ),
    );
    rmSync(lib, { recursive: true, force: true });
    const valid = exported('<div className="bg-primary p-space-4" />');
    writeFileSync(path, valid);
    const [result] = await lint.lintFiles([path]);
    assert.deepEqual(result.messages, []);
    assert.deepEqual(
      ts.getPreEmitDiagnostics(ts.createProgram([path], parsed.options)),
      [],
    );
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});

test("DS02 shared lib cannot hide DOM styling behind an imported ref callback", async () => {
  const folder = mkdtempSync(join(desktopRoot, ".ds-runtime-"));
  const dir = (s) => join(folder, "src/shared", s);
  mkdirSync(dir("lib"), { recursive: true });
  mkdirSync(dir("ds"), { recursive: true });
  const configPath = join(folder, "tsconfig.json");
  writeFileSync(
    configPath,
    JSON.stringify({
      extends: join(desktopRoot, "tsconfig.json"),
      compilerOptions: { rootDir: "src" },
      include: ["src"],
    }),
  );
  writeFileSync(
    dir("lib/paint.ts"),
    'export const paint = (element: Readonly<HTMLDivElement> | null): void => element?.style.setProperty("width", "50%");',
  );
  writeFileSync(dir("lib/index.ts"), 'export { paint } from "./paint";');
  writeFileSync(
    dir("ds/Probe.tsx"),
    'import { paint } from "../lib/index";\n' +
      exported('<div className="h-meter-height bg-primary" ref={paint} />'),
  );
  try {
    const native = spawnSync(
      join(desktopRoot, "node_modules/.bin/tsc"),
      ["-p", configPath],
      { encoding: "utf8" },
    );
    assert.equal(native.status, 0, native.stdout + native.stderr);
    const lint = new ESLint({
      cwd: folder,
      overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
      overrideConfig: [
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
    const results = await lint.lintFiles([
      dir("lib/paint.ts"),
      dir("lib/index.ts"),
      dir("ds/Probe.tsx"),
    ]);
    assert.ok(
      results.some((r) =>
        r.messages.some((m) => m.ruleId === "ds/no-dom-styling"),
      ),
      JSON.stringify(results.flatMap((r) => r.messages)),
    );
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
