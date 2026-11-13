import assert from "node:assert/strict";
import test from "node:test";
import {
  readFileSync,
  writeFileSync,
  mkdirSync,
  mkdtempSync,
  cpSync,
  symlinkSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { compile } from "tailwindcss";
import {
  verifyIndependent,
  blocks,
  expectedVariables,
} from "./ds-independent.mjs";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const read = (path) => readFileSync(join(root, path), "utf8");
const tokens = JSON.parse(read("apps/desktop/design-tokens/tokens.json"));
const cssPath = "apps/desktop/src/shared/ds/tokens.css";

test("independent JSON expectations cover full light system dark manual dark and font faces", () => {
  const css = read(cssPath);
  verifyIndependent(css, tokens);
  const rootBlock = blocks(css).find((b) => b.selector === ":root").body;
  const manual = blocks(css).find(
    (b) => b.selector === ':root[data-theme="dark"]',
  ).body;
  const face = blocks(css).find((b) => b.selector === "@font-face").body;
  for (const variant of [
    css.replace(manual, rootBlock),
    css.replace("prefers-color-scheme: dark", "prefers-color-scheme: light"),
    css.replace(/(--sc-shadow-popover:)[^;]+;/gu, "$1 none;"),
    css.replace(face, face.replace("font-weight: 400;", "font-weight: 700;")),
    css.replace('format("woff2")', 'format("woff")'),
  ])
    assert.throws(() => verifyIndependent(variant, tokens));
  for (const theme of ["light", "dark"])
    assert.ok(Object.keys(expectedVariables(tokens, theme)).length > 140);
});

test("Vite includes src classes but excludes tests and ESLint class probes", () => {
  const scratch = mkdtempSync(join(root, "target", "ds-source-"));
  try {
    const desktop = join(scratch, "apps/desktop");
    mkdirSync(desktop, { recursive: true });
    cpSync(join(root, "apps/desktop/src"), join(desktop, "src"), {
      recursive: true,
    });
    for (const file of ["index.html", "vite.config.mjs", "package.json"])
      cpSync(join(root, "apps/desktop", file), join(desktop, file));
    symlinkSync(
      join(root, "apps/desktop/node_modules"),
      join(desktop, "node_modules"),
    );
    symlinkSync(join(root, "design"), join(scratch, "design"));
    mkdirSync(join(desktop, "tests"));
    mkdirSync(join(desktop, "eslint"));
    writeFileSync(
      join(desktop, "tests/zz-probe.mjs"),
      '"p-[13px] z-50 bg-[#ff0000] absolute fixed table";',
    );
    writeFileSync(join(desktop, "eslint/zz-probe.mjs"), '"m-[17px]";');
    writeFileSync(
      join(desktop, "src/shared/ds/probe.tsx"),
      'export const probe = "p-space-4";',
    );
    const run = spawnSync(
      join(root, "apps/desktop/node_modules/.bin/vite"),
      ["build"],
      { cwd: desktop, encoding: "utf8" },
    );
    assert.equal(run.status, 0, run.stderr);
    const output = readdirSync(join(desktop, "dist/assets"))
      .filter((f) => f.endsWith(".css"))
      .map((f) => readFileSync(join(desktop, "dist/assets", f), "utf8"))
      .join("\n");
    assert.match(output, /\.p-space-4\s*\{/u);
    for (const value of [
      "13px",
      "17px",
      ".z-50",
      ".absolute{",
      ".fixed{",
      ".table{",
    ])
      assert.ok(!output.includes(value), value);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});

test("outline focus has width and style and generated type metadata is complete", async () => {
  const output = (
    await compile(read(cssPath) + "\n@tailwind utilities;")
  ).build(["outline-focus", "text-code"]);
  assert.match(
    output,
    /\.outline-focus\s*\{[^}]*outline-style: solid;[^}]*outline-width: var\(--border-width-focus\);/u,
  );
  assert.match(
    output,
    /\.text-code\s*\{[^}]*font-family: var\(--font-mono\);/u,
  );
  const types = read("apps/desktop/src/shared/ds/tokens.ts");
  assert.match(types, /export type ChartSeriesToken/u);
  assert.match(types, /export type ThemeId/u);
  for (const scale of ["90", "100", "115", "130"])
    assert.ok(read(cssPath).includes(`html[data-text-scale="${scale}"]`));
});

test("generator rejects CSS and TS injection malformed values references and unknown groups before writing", () => {
  const scratch = mkdtempSync(join(root, "target", "ds-input-"));
  const inputPath = "apps/desktop/design-tokens/tokens.json";
  try {
    mkdirSync(join(scratch, "apps/desktop/design-tokens"), { recursive: true });
    symlinkSync(join(root, "node_modules"), join(scratch, "node_modules"));
    const run = () =>
      spawnSync(
        "cargo",
        [
          "run",
          "--locked",
          "--quiet",
          "--manifest-path",
          join(root, "Cargo.toml"),
          "--package",
          "xtask",
          "--",
          "gen-tokens",
        ],
        { cwd: scratch, encoding: "utf8" },
      );
    writeFileSync(join(scratch, inputPath), JSON.stringify(tokens));
    assert.equal(run().status, 0);
    const original = readFileSync(join(scratch, cssPath), "utf8");
    const variants = [
      (t) => {
        t.color.tokens[0].name = 'bg" | string | "x';
      },
      (t) => {
        t.color.tokens[0].value.light = "red; } body { display:none";
      },
      (t) => {
        t.color.tokens[0].value.light = "{missing}";
      },
      (t) => {
        t.color.tokens[0].value.light = "{bg}";
      },
      (t) => {
        t.extra = { tokens: [] };
      },
      (t) => {
        t.color.themes[0].id = 'light" | string | "x';
      },
      (t) => {
        t.spacing.tokens[0].value.light = "1.2.3px";
      },
      (t) => {
        t.opacity.tokens[0].value.dark = "2";
      },
      (t) => {
        t.zIndex.tokens[0].value.light = "+10";
      },
      (t) => {
        t.layout.tokens.find((x) => x.name === "grid-columns").value.light =
          "012";
      },
      (t) => {
        t.spacing.tokens[0].value.light = "04px";
      },
      (t) => {
        t.shadow.tokens[0].value.light = "none; color:red";
      },
      (t) => {
        t.easing.tokens[0].value.light = "linear; color:red";
      },
      (t) => {
        t.type.groups[0].styles[0].name = 'x" | string | "y';
      },
      (t) => {
        t.type.fonts[0].family = 'x"; color:red';
      },
      (t) => {
        t.type.families.sans = "x; color:red";
      },
    ];
    for (const mutate of variants) {
      const changed = structuredClone(tokens);
      mutate(changed);
      writeFileSync(join(scratch, inputPath), JSON.stringify(changed));
      assert.notEqual(run().status, 0, String(mutate));
      assert.equal(readFileSync(join(scratch, cssPath), "utf8"), original);
    }
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});

test("one generated token type consumer passes Knip while unrelated dead code fails", () => {
  const scratch = mkdtempSync(join(root, "target", "ds-knip-"));
  try {
    mkdirSync(join(scratch, "src/shared/ds"), { recursive: true });
    mkdirSync(join(scratch, "src/app"), { recursive: true });
    writeFileSync(
      join(scratch, "package.json"),
      '{"name":"ds-knip-probe","type":"module"}',
    );
    cpSync(
      join(root, "apps/desktop/src/shared/ds/tokens.ts"),
      join(scratch, "src/shared/ds/tokens.ts"),
    );
    writeFileSync(
      join(scratch, "src/app/App.tsx"),
      'import type { ColorToken } from "../shared/ds/tokens";\nexport const App = (value: ColorToken): ColorToken => value;',
    );
    writeFileSync(
      join(scratch, "src/app/main.tsx"),
      'import { App } from "./App"; App("ink");',
    );
    const config = JSON.parse(read("apps/desktop/knip.json"));
    config.entry = ["src/app/main.tsx!"];
    config.ignoreDependencies = [];
    config.ignoreBinaries = [];
    const configPath = join(scratch, "knip.json");
    writeFileSync(configPath, JSON.stringify(config));
    const run = () =>
      spawnSync(
        join(root, "apps/desktop/node_modules/.bin/knip"),
        [
          "--config",
          configPath,
          "--directory",
          scratch,
          "--strict",
          "--reporter",
          "json",
          "--no-progress",
        ],
        { cwd: scratch, encoding: "utf8" },
      );
    let result = run();
    assert.equal(result.status, 0, result.stdout + result.stderr);
    writeFileSync(
      join(scratch, "src/shared/ds/dead.ts"),
      "export const dead = 1;",
    );
    result = run();
    assert.notEqual(result.status, 0);
    assert.match(result.stdout, /src\/shared\/ds\/dead\.ts/u);
    rmSync(join(scratch, "src/shared/ds/dead.ts"));
    writeFileSync(
      join(scratch, "src/shared/ds/live.ts"),
      "export const used = 1; export const unused = 2;",
    );
    writeFileSync(
      join(scratch, "src/app/main.tsx"),
      'import {App} from "./App"; import {used} from "../shared/ds/live"; App("ink"); void used;',
    );
    result = run();
    assert.notEqual(result.status, 0);
    assert.match(result.stdout, /unused/u);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});

test("Tailwind escape hatches remain explicit DS02 responsibilities", async () => {
  const theme =
    read("apps/desktop/node_modules/tailwindcss/theme.css") +
    "\n" +
    read(cssPath) +
    "\n@tailwind utilities;";
  const generated = [
    "p-[13px]",
    "[color:red]",
    "p-(--x)",
    "z-50",
    "opacity-50",
    "duration-150",
    "grid-cols-7",
    "border-2",
    "outline-2",
    "line-clamp-3",
    "w-1/2",
    "w-full",
    "h-screen",
    "rounded-full",
    "bg-primary/50",
    "dark:bg-primary",
    "p-sidebar-width",
    "w-space-4",
  ];
  for (const candidate of generated) {
    const output = (await compile(theme))
      .build([candidate])
      .replace(/\\(.)/gu, "$1");
    assert.ok(output.includes(`.${candidate} {`), candidate);
  }
  for (const candidate of ["bg-red-500", "p-5", "text-sm"]) {
    const output = (await compile(theme))
      .build([candidate])
      .replace(/\\(.)/gu, "$1");
    assert.ok(!output.includes(`.${candidate} {`), candidate);
  }
});
