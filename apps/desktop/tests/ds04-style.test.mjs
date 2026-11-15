import assert from "node:assert/strict";
import { test } from "node:test";
import { ESLint } from "eslint";
import { dsConfigs } from "../eslint/ds-config.mjs";
import { desktopRoot } from "./paths.mjs";
test("DS04 only the artifact backed overlay, skeleton and chart sizes pass style rules", async () => {
  const engine = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: dsConfigs(),
  });
  const lint = async (classes) =>
    (
      await engine.lintText(`const Probe=()=> <div className="${classes}"/>;`, {
        filePath: "src/shared/ds/Probe.tsx",
      })
    )[0].messages;
  assert.deepEqual(
    await lint(
      "dialog-overlay z-modal bg-overlay skeleton-line border-b-hairline h-chart-height-sm h-chart-height h-chart-height-lg",
    ),
    [],
  );
  for (const invalid of [
    "fixed",
    "inset-0",
    "h-chart-height-xl",
    "h-[100px]",
    "z-50",
  ]) {
    assert.ok(
      (await lint(invalid)).some(
        (message) =>
          message.ruleId === "better-tailwindcss/no-restricted-classes",
      ),
      invalid,
    );
  }
});
