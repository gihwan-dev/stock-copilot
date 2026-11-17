import { strict as assert } from "node:assert";
import { test } from "node:test";
import { ESLint } from "eslint";
import { dsConfigs } from "../eslint/ds-config.mjs";
import { desktopRoot } from "./paths.mjs";

const lint = async (source, filePath) => {
  const engine = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: dsConfigs(),
  });
  const [result] = await engine.lintText(source, { filePath });
  return result.messages.map((message) => message.ruleId);
};

test("overflow markers are limited to their owning design-system files", async () => {
  for (const [source, file] of [
    ['const x = <h1 data-truncate="" />;', "src/shared/ds/Card.tsx"],
    ['const x = <div data-scroll-viewport="" />;', "src/app/Screen.tsx"],
    ['const x = {"data-truncate": ""};', "src/features/a/ui/Screen.tsx"],
    [
      'const x = {["data-" + "scroll-viewport"]: ""};',
      "src/shared/ds/Heading.tsx",
    ],
    [
      'const key = "data-truncate"; const x = {[key]: ""};',
      "src/shared/ds/Card.tsx",
    ],
    [
      'const prefix = "data-"; const key = prefix + "truncate"; const x = {[key]: ""};',
      "src/shared/ds/Card.tsx",
    ],
    ['const x = {[`data-${"truncate"}`]: ""};', "src/shared/ds/Card.tsx"],
    [
      'const x = {[["data", "truncate"].join("-")]: ""};',
      "src/shared/ds/Card.tsx",
    ],
  ])
    assert.ok(
      (await lint(source, file)).includes("ds/no-overflow-markers"),
      file,
    );
  assert.ok(
    !(
      await lint('const x = <p data-truncate="" />;', "src/shared/ds/Text.tsx")
    ).includes("ds/no-overflow-markers"),
  );
  assert.ok(
    !(
      await lint(
        'const x = <div data-scroll-viewport="" />;',
        "src/shared/ds/ScrollArea.tsx",
      )
    ).includes("ds/no-overflow-markers"),
  );
  assert.ok(
    !(
      await lint(
        'const x = <h1 data-truncate="" />;',
        "src/shared/ds/Heading.tsx",
      )
    ).includes("ds/no-overflow-markers"),
  );
  assert.ok(
    !(
      await lint(
        'const key = "title"; const x = {[key]: ""};',
        "src/shared/ds/Card.tsx",
      )
    ).includes("ds/no-overflow-markers"),
  );
  assert.ok(
    !(
      await lint(
        'const key = "data-truncate"; function f() { const key = "title"; return {[key]: ""}; }',
        "src/shared/ds/Card.tsx",
      )
    ).includes("ds/no-overflow-markers"),
  );
});
