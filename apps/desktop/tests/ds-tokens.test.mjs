import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  tokenProblems,
  readInputs,
  DERIVED_SPACING,
} from "../../../harness/ds-tokens/check.mjs";

const inputs = () => readInputs();

test("the closed derivation list contains exactly the three documented spacings and rejects each deletion", () => {
  assert.deepEqual(DERIVED_SPACING, [7, 9, 11]);
  for (const index of DERIVED_SPACING) {
    const changed = inputs();
    changed.tokens.spacing.tokens = changed.tokens.spacing.tokens.filter(
      (token) => token.name !== `space-${index}`,
    );
    assert.ok(
      tokenProblems(changed).some((problem) =>
        problem.includes("원본 또는 문서에서 유도한 토큰과 다르다"),
      ),
    );
  }
});

test("changed source values and additions outside the closed derivation list fail exact preservation", () => {
  const original = inputs();
  for (const [group, section] of Object.entries(original.source)) {
    if (!Array.isArray(section?.tokens)) continue;
    for (const sourceToken of section.tokens) {
      const changed = structuredClone(original);
      const token = changed.tokens[group].tokens.find(
        (entry) => entry.name === sourceToken.name,
      );
      token.value.light = "altered";
      assert.ok(
        tokenProblems(changed).some((problem) =>
          problem.includes("원본 또는 문서에서 유도한 토큰과 다르다"),
        ),
        token.name,
      );
    }
    const changed = structuredClone(original);
    changed.tokens[group].tokens.push({
      name: "unapproved-extra",
      value: structuredClone(changed.tokens[group].tokens[0].value),
      usage: "추가 변형",
    });
    assert.ok(
      tokenProblems(changed).some((problem) =>
        problem.includes("원본 또는 문서에서 유도한 토큰과 다르다"),
      ),
      group,
    );
  }
});

test("code tokens preserve the design source and every architecture 5.1 token", () => {
  assert.deepEqual(tokenProblems(inputs()), []);
});

test("removing every token or typography element is rejected", () => {
  const original = inputs();
  for (const [group, section] of Object.entries(original.tokens)) {
    if (!Array.isArray(section?.tokens)) continue;
    section.tokens.forEach((token, index) => {
      const changed = structuredClone(original);
      changed.tokens[group].tokens.splice(index, 1);
      assert.ok(tokenProblems(changed).length > 0, token.name);
    });
  }
  original.tokens.type.groups.forEach((group, index) => {
    group.styles.forEach((style, position) => {
      const changed = structuredClone(original);
      changed.tokens.type.groups[index].styles.splice(position, 1);
      assert.ok(tokenProblems(changed).length > 0, style.name);
    });
  });
});

test("both themes have identical key sets and reject missing or extra themes", () => {
  const original = inputs();
  for (const [group, section] of Object.entries(original.tokens)) {
    if (!Array.isArray(section?.tokens)) continue;
    for (const [index, token] of section.tokens.entries()) {
      assert.deepEqual(Object.keys(token.value).sort(), ["dark", "light"]);
      for (const theme of ["light", "dark"]) {
        const changed = structuredClone(original);
        delete changed.tokens[group].tokens[index].value[theme];
        assert.ok(tokenProblems(changed).length > 0, `${token.name}/${theme}`);
      }
    }
  }
  const changed = structuredClone(original);
  changed.tokens.color.tokens[0].value.extra = "#ffffff";
  assert.ok(tokenProblems(changed).length > 0);
});

test("invalid formats, unknown keys, duplicate names and unresolved or cyclic references fail", () => {
  for (const [group, value] of [
    ["color", "#xyz"],
    ["spacing", "4rem"],
    ["radius", "-4px"],
    ["shadow", "arbitrary"],
    ["size", "NaNpx"],
    ["layout", "12oops"],
    ["zIndex", "1.5"],
    ["duration", "fast"],
    ["easing", "linear"],
    ["opacity", "1.1"],
    ["text", "bold"],
  ]) {
    const changed = inputs();
    changed.tokens[group].tokens[0].value.light = value;
    changed.source[group].tokens[0].value = structuredClone(
      changed.tokens[group].tokens[0].value,
    );
    assert.ok(
      tokenProblems(changed).some((problem) =>
        problem.includes("값 형식 오류"),
      ),
      group,
    );
  }
  for (const reference of ["{missing}", "{bg}"]) {
    const changed = inputs();
    changed.tokens.color.tokens[0].value.light = reference;
    changed.source.color.tokens[0].value.light = reference;
    assert.ok(
      tokenProblems(changed).some((problem) => problem.includes("참조 오류")),
      reference,
    );
  }
  for (const name of ["bad_name", "surface"]) {
    const changed = inputs();
    changed.tokens.color.tokens[0].name = name;
    assert.ok(tokenProblems(changed).length > 0, name);
  }
  const changed = inputs();
  changed.tokens.unrecognized = {};
  assert.ok(tokenProblems(changed).length > 0);
});

test("architecture table additions are required without changing the checker", () => {
  const changed = inputs();
  changed.architecture = changed.architecture.replace(
    "| 화면 가장자리, 섹션 사이 | `space-8` |",
    "| 화면 가장자리, 섹션 사이 | `space-8`, `space-13` |",
  );
  assert.ok(
    tokenProblems(changed).some((problem) => problem.includes("space-13")),
  );
});

test("derived spacing follows the documented four pixel scale", () => {
  const { tokens } = inputs();
  for (let index = 1; index <= 12; index += 1) {
    const token = tokens.spacing.tokens.find(
      (entry) => entry.name === `space-${index}`,
    );
    assert.deepEqual(token.value, {
      light: `${index * 4}px`,
      dark: `${index * 4}px`,
    });
  }
  assert.match(
    readFileSync("docs/adr/ds-01-code-tokens.md", "utf8"),
    /space-7.*space-9.*space-11/u,
  );
});
