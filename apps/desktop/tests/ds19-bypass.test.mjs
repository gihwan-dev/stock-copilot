import assert from "node:assert/strict";
import test from "node:test";
import {
  cpSync,
  mkdirSync,
  writeFileSync,
  rmSync,
  readFileSync,
} from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
test("DS19 actual type-aware app rejects templates helpers aliases and imported semantic mappings", async () => {
  const folder = mkdtempSync("ds19-bypass-");
  try {
    cpSync(join(desktopRoot, "src"), join(folder, "src"), { recursive: true });
    const project = join(folder, "tsconfig.json");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        include: ["src"],
        compilerOptions: {
          rootDir: "src",
          noUnusedLocals: false,
          noUnusedParameters: false,
        },
      }),
    );
    const engine = new ESLint({
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
    const probe = join(folder, "src/shared/ds/Probe.tsx");
    const header = 'import type {ReactElement} from "react";';
    for (const [name, code, wanted, compile] of [
      [
        "template",
        "export const Probe=():ReactElement=><div className={`overflow-hidden`}/>;",
        "ds/static-classes",
      ],
      [
        "template interpolation",
        'const part="hidden";export const Probe=():ReactElement=><div className={`overflow-${part}`}/>;',
        "ds/static-classes",
      ],
      [
        "variable",
        'const value="overflow-hidden";export const Probe=():ReactElement=><div className={value}/>;',
        "ds/static-classes",
      ],
      [
        "assembled variable",
        'const value="overflow-"+"hidden";export const Probe=():ReactElement=><div className={value}/>;',
        "ds/static-classes",
      ],
      [
        "clsx",
        'import clsx from "clsx";export const Probe=():ReactElement=><div className={clsx("overflow-hidden")}/>;',
        "ds/static-classes",
      ],
      [
        "cn literal",
        'export const Probe=():ReactElement=><div className={cn("overflow-hidden")}/>;',
        "better-tailwindcss/no-restricted-classes",
      ],
      [
        "cn variable",
        'const value="overflow-hidden";export const Probe=():ReactElement=><div className={cn(value)}/>;',
        "ds/static-classes",
      ],
      [
        "semantic table",
        'const classes={a:"whitespace-nowrap",b:"text-body"} as const;type Props={readonly variant:"a"|"b"};export const Probe=(props:Props):ReactElement=><div className={classes[props.variant]}/>;',
        "ds/static-classes",
      ],
      [
        "prose token in another file",
        'const classes={a:"max-w-prose-max",b:"text-body"} as const;type Props={readonly variant:"a"|"b"};export const Probe=(props:Props):ReactElement=><div className={classes[props.variant]}/>;',
        "ds/static-classes",
      ],
      [
        "native multiple alias",
        'const Element="select" as const;export const Probe=():ReactElement=><Element multiple/>;',
        "ds/no-presentation-props",
        true,
      ],
      [
        "native dialog alias",
        'const Element="dialog" as const;export const Probe=():ReactElement=><Element open/>;',
        "ds/no-presentation-props",
        true,
      ],
      [
        "native pre alias",
        'const Element="pre" as const;export const Probe=():ReactElement=><Element className="text-body"/>;',
        "ds/no-presentation-props",
        true,
      ],
      [
        "native textarea alias",
        'const Element="textarea" as const;export const Probe=():ReactElement=><Element cols={200}/>;',
        "ds/no-presentation-props",
        true,
      ],
      [
        "named control",
        'export const Probe=():ReactElement=><div className="text-body"/>;',
        null,
      ],
    ]) {
      writeFileSync(probe, header + code);
      if (compile) {
        const compiled = spawnSync(
          join(desktopRoot, "node_modules/.bin/tsc"),
          ["-p", project],
          { encoding: "utf8" },
        );
        assert.equal(
          compiled.status,
          0,
          name + compiled.stdout + compiled.stderr,
        );
      }
      const [result] = await engine.lintFiles([probe]);
      if (wanted === null) assert.deepEqual(result.messages, [], name);
      else
        assert.ok(
          result.messages.some((m) => m.ruleId === wanted),
          `${name}: ${JSON.stringify(result.messages)}`,
        );
    }
    writeFileSync(
      join(folder, "src/shared/ds/button-class-values.ts"),
      'export const classes={a:"h-control-height",b:"h-control-height-sm"} as const;',
    );
    writeFileSync(
      probe,
      header +
        'import {classes} from "./button-class-values";type Props={readonly variant:"a"|"b"};export const Probe=(props:Props):ReactElement=><div className={classes[props.variant]}/>;',
    );
    const built = spawnSync(
      join(desktopRoot, "node_modules/.bin/tsc"),
      ["-p", project],
      { encoding: "utf8" },
    );
    assert.equal(built.status, 0, built.stdout + built.stderr);
    const [result] = await engine.lintFiles([probe]);
    assert.ok(
      result.messages.some((m) => m.ruleId === "ds/static-classes"),
      JSON.stringify(result.messages),
    );
    const chartTableFile = join(folder, "src/shared/ds/ChartTable.tsx");
    const originalChartTable = readFileSync(chartTableFile, "utf8");
    try {
      writeFileSync(
        chartTableFile,
        originalChartTable +
          '\nconst permissionProbeClasses={a:"fixed",b:"text-body"} as const;type PermissionProbeProps={readonly variant:"a"|"b"};export const PermissionProbe=(props:PermissionProbeProps):ReactElement=><div className={permissionProbeClasses[props.variant]}/>;',
      );
      const compiled = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", project],
        { encoding: "utf8" },
      );
      assert.equal(compiled.status, 0, compiled.stdout + compiled.stderr);
      const [rejected] = await engine.lintFiles([chartTableFile]);
      assert.ok(
        rejected.messages.some(
          (message) => message.ruleId === "ds/static-classes",
        ),
        JSON.stringify(rejected.messages),
      );
    } finally {
      writeFileSync(chartTableFile, originalChartTable);
    }
    const privateHeader =
      header +
      'import {EmptyState,Skeleton} from "./index";import {text} from "../lib";';
    for (const name of ["EmptyState", "Skeleton"]) {
      const target = join(folder, `src/shared/ds/${name}.tsx`);
      const original = readFileSync(target, "utf8");
      try {
        writeFileSync(
          target,
          original +
            '\nconst ownClasses={a:"h-control-height",b:"text-body"} as const;type OwnProps={readonly variant:"a"|"b"};export const DS19Map=(props:OwnProps):ReactElement=><div className={ownClasses[props.variant]}/>;',
        );
        const compiled = spawnSync(
          join(desktopRoot, "node_modules/.bin/tsc"),
          ["-p", project],
          { encoding: "utf8" },
        );
        assert.equal(compiled.status, 0, compiled.stdout + compiled.stderr);
        const [result] = await engine.lintFiles([target]);
        assert.ok(
          result.messages.some((m) => m.ruleId === "ds/static-classes"),
          name + JSON.stringify(result.messages),
        );
      } finally {
        writeFileSync(target, original);
      }
    }
    for (const [name, body, valid] of [
      [
        "own component dimensions",
        '<><EmptyState title={text("app.title","ko")}>{text("ds.description","ko")}</EmptyState><Skeleton variant="block" height="chart-height"/><Skeleton lines={6}/></>',
        true,
      ],
      [
        "no arbitrary child",
        '<EmptyState title={text("app.title","ko")}><div/></EmptyState>',
        false,
      ],
      [
        "no description width",
        '<EmptyState title={text("app.title","ko")} width="prose-max"/>',
        false,
      ],
      [
        "no arbitrary skeleton height",
        '<Skeleton variant="block" height="dialog-width"/>',
        false,
      ],
      [
        "no arbitrary skeleton child",
        '<Skeleton variant="block"><div/></Skeleton>',
        false,
      ],
      ["closed line count", "<Skeleton lines={4}/>", false],
    ]) {
      writeFileSync(
        probe,
        privateHeader + "export const Probe=():ReactElement=>" + body + ";",
      );
      const compiled = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", project],
        { encoding: "utf8" },
      );
      assert.equal(
        compiled.status === 0,
        valid,
        name + compiled.stdout + compiled.stderr,
      );
      if (!valid) assert.match(compiled.stdout, /TS(?:2322|2745|2747)/u, name);
    }
    for (const file of ["src/shared/ds/Markdown.tsx", "src/app/Probe.tsx"]) {
      const target = join(folder, file);
      mkdirSync(join(target, ".."), { recursive: true });
      writeFileSync(
        target,
        file.includes("Markdown")
          ? '// eslint-disable-next-line better-tailwindcss/no-restricted-classes -- rule probe\nconst probe=<div className="max-w-dialog-width"/>;'
          : '// eslint-disable-next-line no-restricted-imports -- rule probe\nimport {ScrollArea} from "../shared/ds";',
      );
      const [disabled] = await engine.lintFiles([target]);
      assert.ok(
        disabled.messages.some(
          (m) =>
            m.ruleId ===
            "@eslint-community/eslint-comments/no-restricted-disable",
        ),
        JSON.stringify(disabled.messages),
      );
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
