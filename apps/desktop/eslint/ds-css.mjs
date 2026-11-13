import { readFileSync } from "node:fs";
import stylelint from "stylelint";
import strictValue from "stylelint-declaration-strict-value";
const generated = readFileSync(
  new URL("../src/shared/ds/tokens.css", import.meta.url),
  "utf8",
);
export const CSS_CONFIG = {
  plugins: [strictValue],
  rules: {
    "color-no-hex": true,
    "scale-unlimited/declaration-strict-value": [
      ["/.*/"],
      { disableFix: true },
    ],
  },
};
export const cssProblems = async (
  code,
  config = CSS_CONFIG,
  variables = generated,
) => {
  const result = await stylelint.lint({ code, config });
  const known = new Set(
    [...variables.matchAll(/(--[a-z\d-]+)\s*:/gu)].map((m) => m[1]),
  );
  return [
    ...result.results.flatMap((r) => r.warnings.map((w) => w.rule)),
    ...[...code.matchAll(/var\(\s*(--[a-zA-Z\d_-]+)/gu)]
      .filter((m) => !known.has(m[1]))
      .map((m) => `unknown-var:${m[1]}`),
    ...(/(?:\\|\b(?:calc|min|max|clamp|env|url)\s*\(|\/\*|@(?:import|theme|utility|apply)|--[a-z\d-]+\s*:|var\([^)]*,)/u.test(
      code,
    )
      ? ["unsupported-css"]
      : []),
  ];
};
