import assert from "node:assert/strict";
import test from "node:test";
import { ESLint } from "eslint";
import { dsConfigs, DS_RULES } from "../eslint/ds-config.mjs";
import { desktopRoot } from "./paths.mjs";
const restricted = "better-tailwindcss/no-restricted-classes";
const rules = Object.fromEntries(
  Object.keys(DS_RULES).map((key) => [
    key,
    key === restricted ? DS_RULES[key] : "off",
  ]),
);
const engine = new ESLint({
  cwd: desktopRoot,
  overrideConfigFile: true,
  overrideConfig: dsConfigs(rules),
});
test("DS19 existing application has no unapproved overflow classes", async () => {
  const results = await engine.lintFiles(["src/shared/ds/**/*.tsx"]);
  assert.deepEqual(
    results.flatMap((r) =>
      r.messages
        .filter((m) => m.message.includes("스크롤·넘침"))
        .map((m) => `${r.filePath.split("/").at(-1)}:${m.line}: ${m.message}`),
    ),
    [],
  );
});
