import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import {
  contrast,
  contrastProblems,
  pairProblems,
} from "../../../harness/ds-tokens/contrast.mjs";

const tokens = JSON.parse(
  readFileSync(new URL("../design-tokens/tokens.json", import.meta.url)),
);
const pairs = JSON.parse(
  readFileSync(
    new URL("../../../harness/ds-tokens/contrast-pairs.json", import.meta.url),
  ),
);
const expected = [
  ...["ink", "ink-muted", "ink-subtle", "up", "down"].flatMap((foreground) =>
    ["bg", "surface", "surface-sunken"].map((background) => ({
      foreground,
      background,
      minimum: 4.5,
    })),
  ),
  ...["line-strong", "focus-ring"].flatMap((foreground) =>
    ["bg", "surface", "surface-sunken"].map((background) => ({
      foreground,
      background,
      minimum: 3,
    })),
  ),
  ...["primary", "up", "down", "alert"].flatMap((name) => [
    { foreground: `on-${name}`, background: name, minimum: 4.5 },
    { foreground: name, background: `${name}-soft`, minimum: 4.5 },
  ]),
  ...["primary", "up", "down"].map((name) => ({
    foreground: `on-${name}`,
    background: `${name}-hover`,
    minimum: 4.5,
  })),
  ...["chart-1", "chart-2", "chart-3", "chart-4", "chart-5", "chart-6"].map(
    (foreground) => ({ foreground, background: "surface", minimum: 3 }),
  ),
];

test("contrast pairs are the complete protected contract and removing any pair fails", () => {
  assert.equal(expected.length, 38);
  assert.deepEqual(pairs, expected);
  assert.deepEqual(pairProblems(pairs), []);
  for (let i = 0; i < pairs.length; i++) {
    assert.notDeepEqual(pairProblems(pairs.filter((_, j) => i !== j)), []);
    const changed = structuredClone(pairs);
    changed[i].minimum = 1;
    assert.notDeepEqual(pairProblems(changed), []);
  }
});

test("WCAG known values R14 measurements alpha and raw threshold boundaries", () => {
  assert.equal(contrast("#000000", "#ffffff"), 21);
  assert.equal(contrast("#777777", "#777777"), 1);
  assert.ok(Math.abs(contrast("#777777", "#ffffff") - 4.478089) < 0.000001);
  assert.ok(Math.abs(contrast("#0b7258", "#ffffff") - 5.895447) < 0.000001);
  assert.ok(Math.abs(contrast("#0b7258", "#f5f5f2") - 5.3973) < 0.0001);
  assert.ok(Math.abs(contrast("#8a846f", "#ffffff") - 3.7426) < 0.001);
  assert.equal(contrast("rgba(0, 0, 0, 0)", "#ffffff"), 1);
  assert.equal(contrast("rgba(0, 0, 0, 1)", "#ffffff"), 21);
  assert.ok(
    Math.abs(contrast("rgba(0, 0, 0, 0.5)", "#ffffff") - 3.976653) < 0.000001,
  );
  assert.throws(() => contrast("#000000", "rgba(255, 255, 255, 0.5)"));
  assert.equal(contrast("#000000", "rgba(255, 255, 255, 0.5)", "#ffffff"), 21);
  assert.ok(contrast("#777777", "#ffffff") < 4.5);
  assert.ok(contrast("#767676", "#ffffff") > 4.5);
  const alphaAt = (ratio) =>
    1 - (1.055 * (1.05 / ratio - 0.05) ** (1 / 2.4) - 0.055);
  for (const [ratio, expectedFailure] of [
    [4.49999999, true],
    [4.50000001, false],
  ]) {
    const changed = structuredClone(tokens);
    changed.color.tokens.find((t) => t.name === "ink").value.light =
      `rgba(0, 0, 0, ${alphaAt(ratio)})`;
    assert.equal(
      contrastProblems(changed, pairs).some((p) => p.includes("ink/surface ")),
      expectedFailure,
    );
  }
  const boundary = structuredClone(tokens);
  boundary.color.tokens.find((t) => t.name === "ink").value.light = "#777777";
  assert.ok(
    contrastProblems(boundary, pairs).some((p) => p.includes("ink/surface")),
  );
  boundary.color.tokens.find((t) => t.name === "ink").value.light = "#767676";
  assert.ok(
    !contrastProblems(boundary, pairs).some((p) => p.includes("ink/surface ")),
  );
});

test("both themes pass and low contrast mutations unknown colors cycles and alpha backgrounds fail", () => {
  assert.deepEqual(contrastProblems(tokens, pairs), []);
  for (const theme of ["light", "dark"]) {
    for (const pair of pairs) {
      const changed = structuredClone(tokens);
      changed.color.tokens.find((t) => t.name === pair.foreground).value[
        theme
      ] = `{${pair.background}}`;
      assert.ok(
        contrastProblems(changed, pairs).length > 0,
        `${theme} ${pair.foreground}/${pair.background}`,
      );
    }
  }
  for (const value of [
    "{missing}",
    "{ink}",
    "no-color",
    "rgba(300, 0, 0, 0.5)",
  ]) {
    const changed = structuredClone(tokens);
    changed.color.tokens.find((t) => t.name === "ink").value.light = value;
    assert.ok(contrastProblems(changed, pairs).length > 0, value);
  }
});
