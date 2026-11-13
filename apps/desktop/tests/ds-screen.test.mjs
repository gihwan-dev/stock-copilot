import assert from "node:assert/strict";
import test from "node:test";
import { ESLint } from "eslint";
import { dsConfigs, SCREEN_RULES } from "../eslint/ds-config.mjs";
import { desktopRoot } from "./paths.mjs";

const witnesses = {
  "ds/no-screen-elements": "<div/>",
  "ds/no-screen-props": '<Button className="bg-primary"/>',
  "ds/no-screen-imports": 'import "./style.css";',
  "ds/no-screen-classes": 'const value="hover:bg-primary";',
  "ds/no-screen-format": "const value=new Intl.NumberFormat();",
  "ds/no-ui-attributes": '<Button title="Search"/>',
  "ds/no-dom-values": "const read=(ref)=>ref.current;",
  "ds/no-dom-styling": "<Button dangerouslySetInnerHTML={html}/>",
  "ds/no-element-factory": "const value=createElement(Button);",
  "ds/no-style-object": "const value={className: classes};",
  "ds/no-style-literals": 'const value="13px";',
};
const lint = async (code, rules, path = "src/features/probe/ui/Probe.tsx") => {
  const config = dsConfigs();
  const engine = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: [
      ...config,
      { files: ["src/features/**", "src/app/**"], rules },
    ],
  });
  return (await engine.lintText(code, { filePath: path }))[0].messages.map(
    (m) => m.ruleId,
  );
};
test("DS09 each screen rule loses its witness when disabled", async () => {
  for (const [rule, code] of Object.entries(witnesses)) {
    const isolated = Object.fromEntries(
      Object.keys(SCREEN_RULES).map((key) => [
        key,
        key === rule ? "error" : "off",
      ]),
    );
    assert.ok((await lint(code, isolated)).includes(rule), rule);
    assert.deepEqual(
      await lint(code, { ...isolated, [rule]: "off" }),
      [],
      rule,
    );
  }
});
test("DS09 token class intrinsic composition passes inside DS and fails in both screen roots", async () => {
  const code = '<div className="bg-primary"/>';
  assert.deepEqual(await lint(code, {}, "src/shared/ds/Probe.tsx"), []);
  for (const path of ["src/app/Probe.tsx", "src/features/probe/ui/Probe.tsx"]) {
    const errors = await lint(code, SCREEN_RULES, path);
    for (const rule of ["ds/no-screen-elements", "ds/no-screen-props"])
      assert.ok(errors.includes(rule), rule);
  }
});

test("DS09 screen syntax bypass matrix closes imports props factories and embedding", async () => {
  const cases = [
    ['import "./theme.css";', "ds/no-screen-imports"],
    ['import {css} from "@emotion/react";', "ds/no-screen-imports"],
    ['cn("bg-primary")', "ds/no-screen-imports"],
    ["const value={class:classes};", "ds/no-style-object"],
    ["const value={style:styles};", "ds/no-style-object"],
    ["const value={[key]:classes};", "ds/no-style-object"],
    ["<Button class={classes}/>", "ds/no-screen-props"],
    ["<Button style={styles}/>", "ds/no-screen-props"],
    ["<Button dangerouslySetInnerHTML={html}/>", "ds/no-dom-styling"],
    ["const value=cloneElement(Button);", "ds/no-element-factory"],
    ["const value=jsx(Button);", "ds/no-element-factory"],
    ['const value="p-[13px]";', "ds/no-screen-classes"],
    ["const value=`hover:bg-primary`;", "ds/no-screen-classes"],
    ['const value="rgb(1,2,3)";', "ds/no-style-literals"],
    ["const value=(1).toFixed(2);", "ds/no-screen-format"],
    ["const value=(1).toLocaleString();", "ds/no-screen-format"],
    ["<Button ref={()=>undefined}/>", "ds/no-dom-values"],
    ["<Button><div/></Button>", "ds/no-screen-elements"],
    ["<Button render={()=><svg/>}/>", "ds/no-screen-elements"],
  ];
  for (const [code, rule] of cases)
    assert.ok((await lint(code, SCREEN_RULES)).includes(rule), code);
});
