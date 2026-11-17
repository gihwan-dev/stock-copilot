import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
import { dsConfigs, DS_RULES } from "../eslint/ds-config.mjs";
import {
  PRIMITIVE_EXCEPTIONS,
  PRIMITIVE_OWNERS,
} from "../eslint/ds-overflow-primitives.mjs";
const rule = "ds/no-presentation-props";
const engine = new ESLint({
  cwd: desktopRoot,
  overrideConfigFile: true,
  overrideConfig: dsConfigs(
    Object.fromEntries(
      Object.keys(DS_RULES).map((key) => [key, key === rule ? "error" : "off"]),
    ),
  ),
});
test("DS19 Base UI owners and the single IconButton Tooltip Positioner exception are exact", async () => {
  assert.deepEqual(PRIMITIVE_EXCEPTIONS, [
    [
      "src/shared/ds/IconButton.tsx",
      "@base-ui/react/tooltip",
      "Tooltip.Positioner",
    ],
  ]);
  assert.deepEqual(PRIMITIVE_OWNERS, [
    ["ScrollArea", "scroll-area", "ScrollArea"],
    ["Tooltip", "tooltip", "Tooltip"],
    ["Popover", "popover", "Popover"],
    ["Menu", "menu", "Menu"],
    ["ContextMenu", "context-menu", "Menu"],
    ["NavigationMenu", "navigation-menu", "Menu"],
    ["PreviewCard", "preview-card", "Popover"],
    ["Dialog", "dialog", "Dialog"],
    ["AlertDialog", "alert-dialog", "Dialog"],
    ["Toast", "toast", "Toast"],
    ["Select", "select", "Select"],
    ["Combobox", "combobox", "Combobox"],
    ["Autocomplete", "autocomplete", "Combobox"],
  ]);
  for (const [name, path, owner] of PRIMITIVE_OWNERS) {
    for (const code of [
      `import {${name} as P} from "@base-ui/react/${path}";`,
      `import * as P from "@base-ui/react/${path}";`,
      `export {${name} as P} from "@base-ui/react/${path}";`,
      `export * from "@base-ui/react/${path}";`,
      `export * as P from "@base-ui/react/${path}";`,
      `import {${name} as P} from "@base-ui/react";`,
      `import {"${name}" as P} from "@base-ui/react";`,
      `import * as P from "@base-ui/react";`,
      `const p=import("@base-ui/react/${path}");`,
    ]) {
      const [result] = await engine.lintText(code, {
        filePath: "src/shared/ds/Badge.tsx",
      });
      assert.ok(
        result.messages.some((m) => m.ruleId === rule),
        code,
      );
    }
    const [control] = await engine.lintText(
      `import {${name} as P} from "@base-ui/react/${path}";`,
      { filePath: `src/shared/ds/${owner}.tsx` },
    );
    if (["Select", "Combobox", "Autocomplete"].includes(name))
      assert.ok(
        control.messages.some((m) => m.ruleId === rule),
        path,
      );
    else assert.deepEqual(control.messages, [], path);
  }
  const [existing] = await engine.lintText(
    readFileSync(join(desktopRoot, "src/shared/ds/IconButton.tsx"), "utf8"),
    { filePath: "src/shared/ds/IconButton.tsx" },
  );
  assert.deepEqual(existing.messages, []);
});
test("DS19 compilable primitive aliases reexports and IconButton privilege escapes are rejected", async () => {
  const folder = mkdtempSync("ds19-primitives-");
  try {
    cpSync(join(desktopRoot, "src"), join(folder, "src"), { recursive: true });
    const project = join(folder, "tsconfig.json");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        include: ["src/shared/ds/Badge.tsx", "src/shared/ds/IconButton.tsx"],
        compilerOptions: {
          rootDir: "src",
          noUnusedLocals: false,
          noUnusedParameters: false,
        },
      }),
    );
    const actual = new ESLint({
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
    const header = 'import type {ReactElement} from "react";';
    for (const [file, code, wanted] of [
      [
        "Badge",
        'import {"ScrollArea" as S} from "@base-ui/react";export const Probe=():ReactElement=><S.Root><S.Viewport/></S.Root>;',
        rule,
      ],

      [
        "Badge",
        'import {ScrollArea as S} from "@base-ui/react/scroll-area";export const Probe=():ReactElement=><S.Root><S.Viewport/></S.Root>;',
        rule,
      ],
      [
        "Badge",
        'import * as M from "@base-ui/react/popover";export const Probe=():ReactElement=><M.Popover.Root><M.Popover.Positioner/></M.Popover.Root>;',
        rule,
      ],
      [
        "Badge",
        'import {Popover as P} from "@base-ui/react";export const Probe=():ReactElement=><P.Root><P.Positioner/></P.Root>;',
        rule,
      ],
      ["Badge", 'export {Tooltip as Tip} from "@base-ui/react/tooltip";', rule],
      [
        "IconButton",
        'import {Tooltip as T} from "@base-ui/react/tooltip";export const IconButton=():ReactElement=><T.Root><T.Positioner/></T.Root>;',
        rule,
      ],
      [
        "IconButton",
        'import * as T from "@base-ui/react/tooltip";export const IconButton=():ReactElement=><T.Tooltip.Root><T.Tooltip.Positioner/></T.Tooltip.Root>;',
        rule,
      ],
      [
        "IconButton",
        'export {Tooltip as Tip} from "@base-ui/react/tooltip";export const IconButton=():ReactElement=><div/>;',
        rule,
      ],
      [
        "IconButton",
        'import {Tooltip} from "@base-ui/react/tooltip";const P=Tooltip.Positioner;export const IconButton=():ReactElement=><Tooltip.Root><P/></Tooltip.Root>;',
        rule,
      ],
      [
        "IconButton",
        'import {Tooltip} from "@base-ui/react/tooltip";export const IconButton=():ReactElement=><Tooltip.Root><Tooltip.Positioner/><Tooltip.Positioner/></Tooltip.Root>;',
        rule,
      ],
      [
        "IconButton",
        'import {Popover} from "@base-ui/react/popover";export const IconButton=():ReactElement=><Popover.Root><Popover.Positioner/></Popover.Root>;',
        rule,
      ],
      [
        "IconButton",
        'import {ScrollArea} from "@base-ui/react/scroll-area";export const IconButton=():ReactElement=><ScrollArea.Root><ScrollArea.Viewport/></ScrollArea.Root>;',
        rule,
      ],
      [
        "IconButton",
        'export const IconButton=():ReactElement=><div className="absolute fixed sticky"/>;',
        "better-tailwindcss/no-restricted-classes",
      ],
      [
        "IconButton",
        'import {Tooltip} from "@base-ui/react/tooltip";export const IconButton=():ReactElement=><Tooltip.Root><Tooltip.Positioner/></Tooltip.Root>;',
        null,
      ],
    ]) {
      const target = join(folder, `src/shared/ds/${file}.tsx`);
      const original = readFileSync(target, "utf8");
      try {
        writeFileSync(target, header + code);
        const compiled = spawnSync(
          join(desktopRoot, "node_modules/.bin/tsc"),
          ["-p", project],
          { encoding: "utf8" },
        );
        assert.equal(
          compiled.status,
          0,
          code + compiled.stdout + compiled.stderr,
        );
        const [result] = await actual.lintFiles([target]);
        if (wanted === null) assert.deepEqual(result.messages, [], code);
        else
          assert.ok(
            result.messages.some((m) => m.ruleId === wanted),
            code + JSON.stringify(result.messages),
          );
      } finally {
        writeFileSync(target, original);
      }
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
