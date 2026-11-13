import assert from "node:assert/strict";
import test from "node:test";
import {
  readInputs,
  tokenProblems,
} from "../../../harness/ds-tokens/check.mjs";

const hasProblem = (input, message) =>
  assert.ok(
    tokenProblems(input).some((problem) => problem.includes(message)),
    message,
  );
const paired = (group, name, change) => {
  const input = readInputs();
  change(input.tokens[group].tokens.find((token) => token.name === name));
  input.source[group].tokens = input.tokens[group].tokens.filter(
    (token) => !["space-7", "space-9", "space-11"].includes(token.name),
  );
  return input;
};

test("architecture boundaries and code names must remain readable", () => {
  for (const transform of [
    () => "",
    (text) => text.replace("### 5.1 레이아웃 시스템", "### 5.1 레이아웃"),
    (text) => text.replaceAll("`", ""),
    (text) => text.replace("### 5.2", "### 5.2x"),
  ]) {
    const input = readInputs();
    input.architecture = transform(input.architecture);
    hasProblem(input, "architecture 5.1");
  }
});

test("architecture name counts and range notation are pinned", () => {
  const input = readInputs();
  input.architecture = input.architecture.replace(
    "`gap-hairline`",
    "gap-hairline",
  );
  hasProblem(input, "이름 수");
  const range = readInputs();
  range.architecture = range.architecture.replace(
    "`space-1`~`space-12`",
    "`space-1`부터 `space-12`",
  );
  hasProblem(range, "간격 범위");
});

test("all architecture code names outside the closed exclusions are required", () => {
  for (const name of [
    "icon-xl",
    "z-overlay",
    "border-thick",
    "radius-xl",
    "shadow-dialog",
  ]) {
    const input = readInputs();
    input.architecture = input.architecture.replace(
      "| 표 셀 좌우 | `space-3` |",
      `| 표 셀 좌우 | \`space-3\`, \`${name}\` |`,
    );
    hasProblem(input, `필수 토큰 누락: ${name}`);
  }
});

test("fixed wildcard expansions and spacing ranges survive deletions from both inputs", () => {
  for (const [group, name] of [
    ["size", "chart-height-lg"],
    ["zIndex", "z-popover"],
    ["spacing", "space-5"],
  ]) {
    const input = readInputs();
    for (const data of [input.source, input.tokens])
      data[group].tokens = data[group].tokens.filter(
        (token) => token.name !== name,
      );
    hasProblem(input, `필수 토큰 누락: ${name}`);
  }
});

test("paired inputs cannot change spacing scale or breakpoint and stacking order", () => {
  hasProblem(
    paired("spacing", "space-3", (token) => {
      token.value = { light: "16px", dark: "16px" };
    }),
    "간격 배율",
  );
  for (const name of ["bp-window-md", "bp-content-medium"]) {
    hasProblem(
      paired("layout", name, (token) => {
        token.value = { light: "1400px", dark: "1400px" };
      }),
      "구간 순서",
    );
  }
  hasProblem(
    paired("zIndex", "z-panel", (token) => {
      token.value = { light: "500", dark: "500" };
    }),
    "겹침 순서",
  );
});

test("paired malformed names and duplicates report their own diagnostic", () => {
  for (const name of ["bad_name", "surface"])
    hasProblem(
      paired("color", "bg", (token) => {
        token.name = name;
      }),
      "잘못되거나 중복된 이름",
    );
});

test("paired missing and extra theme keys report their own diagnostic", () => {
  hasProblem(
    paired("color", "bg", (token) => {
      delete token.value.dark;
    }),
    "테마 키 불일치",
  );
  hasProblem(
    paired("color", "bg", (token) => {
      token.value.extra = "#ffffff";
    }),
    "테마 키 불일치",
  );
});

test("paired typography and font formats and family references report their own diagnostic", () => {
  for (const [field, value] of [
    ["fontSize", "big"],
    ["lineHeight", "normal"],
    ["fontWeight", 450],
    ["letterSpacing", "normal"],
  ]) {
    const input = readInputs();
    for (const data of [input.source, input.tokens])
      data.type.groups[0].styles[0][field] = value;
    hasProblem(input, "글자 형식 오류");
  }
  const family = readInputs();
  for (const data of [family.source, family.tokens])
    data.type.groups[0].family = "absent";
  hasProblem(family, "글꼴 참조 오류");
  for (const [field, value] of [
    ["file", "../outside.woff2"],
    ["weight", "450"],
    ["style", "italic"],
  ]) {
    const input = readInputs();
    for (const data of [input.source, input.tokens])
      data.type.fonts[0][field] = value;
    hasProblem(input, "글꼴 형식 오류");
  }
});

test("font paths must name actual files under design", () => {
  const input = readInputs();
  for (const data of [input.source, input.tokens])
    data.type.fonts[0].file = "fonts/Missing.woff2";
  hasProblem(input, "글꼴 파일 없음");
});
