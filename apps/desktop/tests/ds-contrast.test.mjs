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
test("contrast pairs are the complete protected contract and removing any pair fails", () => {
  assert.equal(pairs.pairs.length, 65);
  assert.deepEqual(pairProblems(pairs), []);
  for (let i = 0; i < pairs.pairs.length; i++) {
    const changed = structuredClone(pairs);
    changed.pairs.splice(i, 1);
    assert.notDeepEqual(pairProblems(changed), []);
  }
  for (const field of ["exclusions", "pending"])
    for (let i = 0; i < pairs[field].length; i++) {
      const changed = structuredClone(pairs);
      changed[field].splice(i, 1);
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
    for (const pair of pairs.pairs) {
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

test("raised surfaces unknown colors and closed pending exceptions cannot pass silently", () => {
  const changed = structuredClone(tokens);
  changed.color.tokens.find((t) => t.name === "surface-raised").value.dark =
    "{ink}";
  assert.ok(
    contrastProblems(changed, pairs).some((p) =>
      p.includes("ink/surface-raised"),
    ),
  );
  const added = structuredClone(tokens);
  added.color.tokens.push({
    name: "new-color",
    value: { light: "#000000", dark: "#ffffff" },
  });
  assert.ok(
    contrastProblems(added, pairs).some((p) => p.includes("new-color")),
  );
  for (const pending of pairs.pending) {
    const fixed = structuredClone(tokens);
    fixed.color.tokens.find((t) => t.name === pending.foreground).value[
      pending.theme
    ] = "#ffffff";
    assert.ok(
      contrastProblems(fixed, pairs).some((p) => p.includes("확인 대기 목록")),
    );
    const worsened = structuredClone(tokens);
    worsened.color.tokens.find((t) => t.name === pending.foreground).value[
      pending.theme
    ] = "#777777";
    assert.ok(contrastProblems(worsened, pairs).length > 0);
  }
});
