import assert from "node:assert/strict";

const normalized = (value) => value.replace(/\s+/gu, " ").trim();
const declarations = (body) =>
  Object.fromEntries(
    body
      .split(";")
      .filter((s) => s.trim())
      .map((s) => {
        const at = s.indexOf(":");
        return [s.slice(0, at).trim(), normalized(s.slice(at + 1))];
      }),
  );
export const blocks = (source) => {
  const result = [];
  let start = 0,
    depth = 0,
    open = 0;
  for (let i = 0; i < source.length; i++) {
    if (source[i] === "{") {
      if (depth === 0) open = i;
      depth++;
    }
    if (source[i] === "}") {
      depth--;
      if (depth === 0) {
        result.push({
          selector: normalized(source.slice(start, open)),
          body: source.slice(open + 1, i),
        });
        start = i + 1;
      }
    }
  }
  assert.equal(depth, 0);
  return result;
};
export const expectedVariables = (tokens, theme) => {
  const result = {};
  const length = (value) => `${Number(value.slice(0, -2)) / 16}rem`;
  for (const [group, data] of Object.entries(tokens))
    for (const token of data.tokens ?? []) {
      const n = token.name,
        v = token.value[theme];
      let key,
        value = v;
      if (group === "color") {
        key = `--color-${n}`;
        value = v.startsWith("{") ? `var(--color-${v.slice(1, -1)})` : v;
      } else if (group === "shadow") {
        key = `--${n}`;
        value = `var(--sc-${n})`;
        result[`--sc-${n}`] = v;
      } else if (group === "spacing") {
        key = `--spacing-${n}`;
        value = length(v);
      } else if (group === "radius") {
        key = `--${n}`;
        value = length(v);
      } else if (group === "duration") key = `--transition-${n}`;
      else if (group === "easing" || group === "opacity") key = `--${n}`;
      else if (group === "zIndex") key = `--z-index-${n.slice(2)}`;
      else if (n.startsWith("border-")) key = `--border-width-${n.slice(7)}`;
      else if (n === "focus-offset" || n === "grid-columns") key = `--sc-${n}`;
      else if (group === "text") {
        key = n.startsWith("text-")
          ? `--text-size-${n.slice(5)}`
          : n.startsWith("weight-")
            ? `--font-${n}`
            : `--${n}`;
        if (!n.startsWith("weight-")) value = length(v);
      } else if (n.startsWith("bp-")) {
        key = n.startsWith("bp-window")
          ? `--breakpoint-${n.slice(3)}`
          : `--container-${n.slice(3)}`;
        value = length(v);
      } else {
        key = `--spacing-${n}`;
        value = v.endsWith("%") ? v : length(v);
      }
      result[key] = normalized(value);
    }
  for (const [name, value] of Object.entries(tokens.type.families))
    result[`--font-${name}`] = normalized(value);
  for (const group of tokens.type.groups)
    for (const style of group.styles) {
      const prefix = `--text-${style.name}`;
      result[prefix] = length(style.fontSize);
      result[`${prefix}--line-height`] = length(style.lineHeight);
      result[`${prefix}--font-weight`] = String(style.fontWeight);
      if (style.letterSpacing !== undefined)
        result[`${prefix}--letter-spacing`] = length(style.letterSpacing);
    }
  return result;
};
export const verifyIndependent = (source, tokens) => {
  const top = blocks(source);
  const find = (list, selector) => {
    const matches = list.filter((b) => b.selector === selector);
    assert.equal(matches.length, 1, selector);
    return matches[0];
  };
  assert.deepEqual(
    declarations(find(top, ":root").body),
    expectedVariables(tokens, "light"),
  );
  const system = blocks(find(top, "@media (prefers-color-scheme: dark)").body);
  assert.deepEqual(
    declarations(find(system, ":root:not([data-theme])").body),
    expectedVariables(tokens, "dark"),
  );
  assert.deepEqual(
    declarations(find(top, ':root[data-theme="dark"]').body),
    expectedVariables(tokens, "dark"),
  );
  const faces = top
    .filter((b) => b.selector === "@font-face")
    .map((b) => declarations(b.body));
  assert.deepEqual(
    faces,
    tokens.type.fonts.map((f) => ({
      "font-family": `"${f.family}"`,
      src: `url("../../../../../design/${f.file}") format("woff2")`,
      "font-weight": f.weight,
      "font-style": f.style,
      "font-display": "swap",
    })),
  );
  for (const group of tokens.type.groups)
    for (const style of group.styles) {
      assert.equal(
        declarations(find(top, `@utility text-${style.name}`).body)[
          "font-family"
        ],
        `var(--font-${group.family})`,
      );
    }
};
