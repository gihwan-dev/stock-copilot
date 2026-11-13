import assert from "node:assert/strict";
import test from "node:test";
import { ESLint } from "eslint";
import { dsConfigs } from "../eslint/ds-config.mjs";
import { styleLiteral } from "../eslint/ds-style-values.mjs";
import { desktopRoot } from "./paths.mjs";

const lint = async (code, filePath = "src/shared/ds/Probe.tsx", lists) => {
  const e = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: dsConfigs(undefined, lists),
  });
  return (await e.lintText(code, { filePath }))[0].messages.map(
    (m) => m.ruleId,
  );
};
test("DS02 final review rejects DOM entry routes and presentation elements", async () => {
  for (const code of [
    "const v=<div ref={paint}/>;",
    'const v=<div ref={(el)=>el?.setHTMLUnsafe("<style/>")}/>;',
    "const v=ref.current;",
    "const v=event.currentTarget;",
    "const v=event.target;",
    "const v=event.nativeEvent;",
  ])
    assert.ok((await lint(code)).includes("ds/no-dom-values"), code);
  for (const tag of [
    "animate",
    "animateTransform",
    "animateMotion",
    "set",
    "font",
    "center",
    "b",
    "i",
    "u",
    "iframe",
    "object",
    "embed",
    "foreignObject",
  ])
    assert.ok(
      (await lint(`const v=<${tag}/>;`)).includes("ds/no-dom-styling"),
      tag,
    );
  assert.ok(
    (await lint('const v="#f00";', "src/shared/lib/value.ts")).includes(
      "ds/no-style-literals",
    ),
  );
});
test("DS02 final review accepts language tag names anchors and trims CSS values", () => {
  for (const value of ["fr", "span", "em", "s", "PT1S"])
    assert.equal(styleLiteral(value), false, value);
  for (const value of [" red", "red ", "13px", "1px 2rem", "rgb(0 0 0)"])
    assert.equal(styleLiteral(value), true, value);
});
test("DS02 final review permits Intl style options and restricts icon geometry to approved files", async () => {
  assert.deepEqual(
    await lint(
      'const v=new Intl.NumberFormat("fr",{style:"unit"});',
      "src/shared/lib/format.ts",
    ),
    [],
  );
  assert.deepEqual(await lint('const v=<a href="#add" lang="fr"/>;'), []);
  const { ICON_GEOMETRY_FILES, ...base } =
    await import("../eslint/ds-allowlists.mjs");
  assert.deepEqual(ICON_GEOMETRY_FILES, []);
  const code =
    'const v=<svg className="fill-none stroke-current stroke-icon stroke-linecap-round stroke-linejoin-round" viewBox="0 0 24 24"><rect width={18} height={18} x={3} y={3} rx={2}/><circle cx={12} cy={12} r={4}/></svg>;';
  assert.deepEqual(
    await lint(code, "src/shared/ds/icon-paths.tsx", {
      ...base,
      ICON_GEOMETRY_FILES: ["src/shared/ds/icon-paths.tsx"],
    }),
    [],
  );
  assert.ok((await lint(code)).includes("ds/no-presentation-props"));
});

test("DS02 final review derives every design length use and pins HTML and Vite CSS inputs", async () => {
  const { readFileSync } = await import("node:fs");
  const { repoRoot } = await import("./paths.mjs");
  const { designLengthClasses } = await import("../eslint/ds-design-usage.mjs");
  const { DS_CLASSES } = await import("../eslint/ds-classes.mjs");
  const { INDEX_HTML, surfaceProblems } =
    await import("../eslint/ds-surfaces.mjs");
  const tokens = JSON.parse(
    readFileSync(`${desktopRoot}/design-tokens/tokens.json`, "utf8"),
  );
  assert.deepEqual(
    tokens.size.tokens.find((t) => t.name === "border-focus").value,
    { light: "2px", dark: "2px" },
  );
  for (const row of designLengthClasses(tokens))
    assert.ok(DS_CLASSES.includes(row.name), JSON.stringify(row));
  for (const name of [
    "w-control-height",
    "size-control-height",
    "min-w-gap-hairline",
  ])
    assert.ok(DS_CLASSES.includes(name), name);
  const html = readFileSync(`${desktopRoot}/index.html`, "utf8"),
    vite = readFileSync(`${desktopRoot}/vite.config.mjs`, "utf8");
  assert.equal(html, INDEX_HTML);
  assert.deepEqual(surfaceProblems(html, vite), []);
  for (const addition of [
    "<style>body{color:red}</style>",
    '<link rel="stylesheet" href="evil.css">',
    '<body style="color:red">',
  ])
    assert.ok(
      surfaceProblems(html + addition, vite).includes("index.html changed"),
    );
  for (const addition of [
    "css: {},",
    '["css"]: {},',
    "css: {postcss:{plugins:[]}},",
    "...extra,",
  ])
    assert.ok(
      surfaceProblems(
        html,
        vite.replace("defineConfig({", `defineConfig({${addition}`),
      ).length > 0,
      addition,
    );
  const writable = readFileSync(
    `${repoRoot}/harness/agent-writable.txt`,
    "utf8",
  ).split("\n");
  assert.equal(writable.includes("apps/desktop/index.html"), false);
});

test("DS02 final review inspects cooked templates and the DOM gate loses its witness when disabled", async () => {
  for (const code of [
    'const v=" red";',
    "const v=`\\u0072ed`;",
    "const v=`\\u0031\\u0033px`;",
  ])
    assert.ok((await lint(code)).includes("ds/no-style-literals"), code);
  const { DS_RULES } = await import("../eslint/ds-config.mjs");
  const rules = Object.fromEntries(
    Object.keys(DS_RULES).map((r) => [
      r,
      r === "ds/no-dom-values" ? "error" : "off",
    ]),
  );
  const code = "const v=<div ref={paint}/>;",
    filePath = "src/shared/ds/Probe.tsx";
  const configs = dsConfigs(rules);
  const on = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: configs,
  });
  assert.deepEqual(
    (await on.lintText(code, { filePath }))[0].messages.map((m) => m.ruleId),
    ["ds/no-dom-values"],
  );
  configs[0].rules["ds/no-dom-values"] = "off";
  const off = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: configs,
  });
  assert.deepEqual((await off.lintText(code, { filePath }))[0].messages, []);
});
