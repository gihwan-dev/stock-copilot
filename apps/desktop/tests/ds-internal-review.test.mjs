import assert from "node:assert/strict";
import test from "node:test";
import { ESLint } from "eslint";
import { dsConfigs } from "../eslint/ds-config.mjs";
import { desktopRoot } from "./paths.mjs";

const lint = async (code, filePath = "src/shared/ds/Probe.tsx") => {
  const engine = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: dsConfigs(),
  });
  const [result] = await engine.lintText(code, { filePath });
  return result.messages.map((m) => m.ruleId);
};
test("DS02 review closes DOM styling and HTML injection members", async () => {
  for (const member of [
    "classList",
    "className",
    "style",
    "setAttribute",
    "setAttributeNS",
    "toggleAttribute",
    "setProperty",
    "cssText",
    "animate",
    "insertAdjacentHTML",
    "innerHTML",
    "outerHTML",
    "adoptedStyleSheets",
    "insertRule",
    "attributeStyleMap",
  ])
    assert.ok(
      (await lint(`const value = element.${member};`)).includes(
        "ds/no-dom-styling",
      ),
      member,
    );
  for (const code of [
    "const value = CSSStyleSheet;",
    "const value = <style />;",
    "const value = <link />;",
    'const value = <div ref={(el) => el?.style.setProperty("width", `${p.pct}%`)} />;',
  ])
    assert.ok((await lint(code)).includes("ds/no-dom-styling"), code);
});
test("DS02 review closes shared JSX React factories and object style keys", async () => {
  for (const folder of ["lib", "ipc", "platform"])
    assert.ok(
      (
        await lint("const value = <div />;", `src/shared/${folder}/Probe.tsx`)
      ).includes("ds/no-shared-jsx"),
      folder,
    );
  for (const code of [
    'import {createElement as make} from "react";',
    'const value = createElement("div", {className:"flex"});',
    'const value = {className:"flex"};',
    "const value = {[`className`]: value};",
    'const value = {["class"+"Name"]: value};',
    "const value = Object.fromEntries(entries);",
  ])
    assert.ok(
      (await lint(code)).some((r) =>
        ["ds/no-element-factory", "ds/no-style-object"].includes(r),
      ),
      code,
    );
});
test("DS02 review blocks presentation attributes but permits SVG geometry", async () => {
  for (const attr of [
    "fill",
    "stroke",
    "strokeWidth",
    "width",
    "height",
    "rx",
    "ry",
    "r",
    "opacity",
    "fillOpacity",
    "strokeOpacity",
    "color",
    "bgcolor",
    "align",
    "size",
  ])
    assert.ok(
      (await lint(`const value = <svg ${attr}={13} />;`)).includes(
        "ds/no-presentation-props",
      ),
      attr,
    );
  assert.ok(
    (
      await lint(
        'const value = <svg viewBox="0 0 24 24"><path d="M0 0L1 1" /></svg>;',
      )
    ).includes("ds/no-presentation-props"),
  );
});
test("DS02 review covers color names functions units and assembled lengths", async () => {
  for (const code of [
    '"red"',
    '"rebeccapurple"',
    '"transparent"',
    '"currentColor"',
    '"color-mix()"',
    '"lab()"',
    '"hwb()"',
    '"light-dark()"',
    '"50%"',
    '"0.8rem"',
    '"1em"',
    '"100vh"',
    '"13pt"',
    '"calc()"',
    '"1"+"3"+"px"',
    'String(13)+"px"',
    "`${n}px`",
  ])
    assert.ok(
      (await lint(`const value = ${code};`)).includes("ds/no-style-literals"),
      code,
    );
});

test("DS02 price inline style and DOM exceptions are protected empty file lists", async () => {
  const lists = await import("../eslint/ds-allowlists.mjs");
  assert.deepEqual(
    { ...lists },
    {
      PRICE_COLOR_FILES: [],
      INLINE_STYLE_FILES: [],
      DOM_STYLE_FILES: [],
      DOM_ACCESS_FILES: ["src/shared/platform/index.ts"],
      ICON_GEOMETRY_FILES: [],
    },
  );
  for (const [key, code] of [
    ["PRICE_COLOR_FILES", 'const value = <div className="text-up" />;'],
    ["INLINE_STYLE_FILES", "const value = <div style={value} />;"],
    ["DOM_STYLE_FILES", "const value = element.style;"],
  ]) {
    const filePath = "src/shared/ds/Approved.tsx";
    const { DS_RULES } = await import("../eslint/ds-config.mjs");
    const engine = new ESLint({
      cwd: desktopRoot,
      overrideConfigFile: true,
      overrideConfig: dsConfigs(DS_RULES, { ...lists, [key]: [filePath] }),
    });
    const [result] = await engine.lintText(code, { filePath });
    assert.deepEqual(result.messages, [], key);
    assert.notDeepEqual(await lint(code), [], key);
  }
});

test("DS02 every new boundary rule loses a witness when switched off", async () => {
  const { DS_RULES, SHARED_RULES } = await import("../eslint/ds-config.mjs");
  for (const [rule, code, filePath] of [
    [
      "ds/no-dom-values",
      "const value = ref.current;",
      "src/shared/ds/Probe.tsx",
    ],
    [
      "ds/no-dom-styling",
      "const value = element.style;",
      "src/shared/ds/Probe.tsx",
    ],
    [
      "ds/no-element-factory",
      'const value = createElement("div");',
      "src/shared/ds/Probe.tsx",
    ],
    [
      "ds/no-style-object",
      'const value = {className:"flex"};',
      "src/shared/ds/Probe.tsx",
    ],
    [
      "ds/no-presentation-props",
      "const value = <svg width={13} />;",
      "src/shared/ds/Probe.tsx",
    ],
    ["ds/no-shared-jsx", "const value = <div />;", "src/shared/lib/Probe.tsx"],
  ]) {
    const configs = dsConfigs(
      Object.fromEntries(
        Object.keys(DS_RULES).map((r) => [r, r === rule ? "error" : "off"]),
      ),
    );
    configs[1].rules = Object.fromEntries(
      Object.keys(SHARED_RULES).map((r) => [r, r === rule ? "error" : "off"]),
    );
    const engine = new ESLint({
      cwd: desktopRoot,
      overrideConfigFile: true,
      overrideConfig: configs,
    });
    const [enabled] = await engine.lintText(code, { filePath });
    assert.deepEqual(
      enabled.messages.map((m) => m.ruleId),
      [rule],
    );
    for (const config of configs)
      if (config.rules?.[rule] !== undefined) config.rules[rule] = "off";
    const off = new ESLint({
      cwd: desktopRoot,
      overrideConfigFile: true,
      overrideConfig: configs,
    });
    const [disabled] = await off.lintText(code, { filePath });
    assert.deepEqual(disabled.messages, []);
  }
});
