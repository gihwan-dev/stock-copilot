import assert from "node:assert/strict";
import test from "node:test";
import {
  cpSync,
  readFileSync,
  writeFileSync,
  rmSync,
  readdirSync,
} from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
import { PRIMITIVE_OWNERS } from "../eslint/ds-overflow-primitives.mjs";
test("DS19 public Base UI entries with position or scroll implementations have an explicit policy", () => {
  const root = join(desktopRoot, "node_modules/@base-ui/react");
  const exports = Object.keys(
    JSON.parse(readFileSync(join(root, "package.json"), "utf8")).exports,
  );
  const files = (dir) =>
    readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
      entry.isDirectory()
        ? files(join(dir, entry.name))
        : entry.name.endsWith(".mjs")
          ? [join(dir, entry.name)]
          : [],
    );
  const risky = exports.filter(
    (path) =>
      /^\.\/[a-z-]+$/u.test(path) &&
      !path.includes("use-") &&
      files(join(root, path.slice(2))).some((file) =>
        /position\s*:\s*["'](?:fixed|absolute|sticky)["']|position:[^;]{0,16}(?:fixed|absolute|sticky)|overflow(?:X|Y)?\s*:\s*["'](?:auto|scroll|hidden|clip)["']|useAnchorPositioning/u.test(
          readFileSync(file, "utf8"),
        ),
      ),
  );
  assert.ok(risky.includes("./number-field"));
  assert.ok(risky.includes("./slider"));
  assert.ok(risky.includes("./drawer"));
  for (const path of risky)
    assert.ok(
      PRIMITIVE_OWNERS.some(([, family]) => path === `./${family}`),
      path,
    );
});
test("DS19 full app compilation witnesses cannot launder primitives or forward popover attributes", async () => {
  const folder = mkdtempSync("ds19-review-three-");
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
    const card = join(folder, "src/shared/ds/Card.tsx"),
      original = readFileSync(card, "utf8");
    const index = join(folder, "src/shared/lib/index.ts"),
      originalIndex = readFileSync(index, "utf8");
    const primitive = join(folder, "src/shared/lib/primitives.ts");
    const compile = () => {
      const r = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", project],
        { encoding: "utf8" },
      );
      assert.equal(r.status, 0, r.stdout + r.stderr);
    };
    const probe = (imports, body) =>
      writeFileSync(
        card,
        imports +
          original.replace("{props.children}", `{props.children}${body}`),
      );
    writeFileSync(
      primitive,
      'export {ScrollArea} from "@base-ui/react/scroll-area";export {Tooltip} from "@base-ui/react/tooltip";',
    );
    writeFileSync(
      index,
      originalIndex + '\nexport {ScrollArea,Tooltip} from "./primitives";',
    );
    probe(
      'import {ScrollArea,Tooltip} from "../lib";',
      "<ScrollArea.Root><ScrollArea.Viewport/></ScrollArea.Root><Tooltip.Root><Tooltip.Positioner/></Tooltip.Root>",
    );
    compile();
    const chain = await actual.lintFiles([primitive, index, card]);
    assert.ok(
      chain[0].messages.some((m) => m.ruleId === "boundaries/dependencies"),
      JSON.stringify(chain.map((r) => r.messages)),
    );
    writeFileSync(card, original);
    writeFileSync(index, originalIndex);
    rmSync(primitive);
    for (const [imports, body] of [
      [
        'import {NumberField} from "@base-ui/react/number-field";',
        "<NumberField.Root><NumberField.ScrubArea><NumberField.ScrubAreaCursor/></NumberField.ScrubArea></NumberField.Root>",
      ],
      [
        'import {Slider} from "@base-ui/react/slider";',
        "<Slider.Root><Slider.Thumb/></Slider.Root>",
      ],
      [
        'import {Drawer} from "@base-ui/react";',
        "<Drawer.Root><Drawer.Portal><Drawer.Backdrop/><Drawer.Popup/></Drawer.Portal></Drawer.Root>",
      ],
      [
        'import {Button as B} from "@base-ui/react/button";',
        '<B popoverTarget="ds19"/><B id="ds19" popover="auto"/>',
      ],
      [
        'import {Button as B} from "@base-ui/react/button";',
        '<B popoverTargetAction="toggle"/>',
      ],
    ]) {
      probe(imports, body);
      compile();
      const [result] = await actual.lintFiles([card]);
      assert.ok(
        result.messages.some((m) => m.ruleId === "ds/no-presentation-props"),
        imports + JSON.stringify(result.messages),
      );
    }
    for (const [file, from, to] of [
      [
        "Card",
        "bg-surface border-hairline",
        "h-chart-height max-w-content-max w-sidebar-width bg-surface border-hairline",
      ],
      [
        "Icon",
        "inline-block flex-none",
        "max-w-content-max h-chart-height-lg inline-block flex-none",
      ],
    ]) {
      const path = join(folder, `src/shared/ds/${file}.tsx`),
        source = readFileSync(
          join(desktopRoot, `src/shared/ds/${file}.tsx`),
          "utf8",
        );
      writeFileSync(card, original);
      writeFileSync(path, source.replace(from, to));
      compile();
      const [result] = await actual.lintFiles([path]);
      assert.ok(
        result.messages.some(
          (m) => m.ruleId === "better-tailwindcss/no-restricted-classes",
        ),
        file + JSON.stringify(result.messages),
      );
      writeFileSync(path, source);
    }
    writeFileSync(card, original);
    const controls = await actual.lintFiles([
      card,
      join(folder, "src/shared/ds/Icon.tsx"),
      join(folder, "src/shared/ds/IconButton.tsx"),
    ]);
    assert.deepEqual(
      controls.flatMap((r) => r.messages),
      [],
    );
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
