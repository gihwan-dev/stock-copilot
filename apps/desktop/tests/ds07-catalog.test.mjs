import assert from "node:assert/strict";
import test from "node:test";
import { catalogProblems } from "../../../harness/ds-catalog/rules.mjs";
const render = () => null;
const sample = {
  sample: 0,
  component: "Card",
  themes: ["light", "dark"],
  render,
};
const entries = ["light", "dark"].map((theme) => ({
  sample: 0,
  title: "Card",
  observedComponent: "Card",
  observedTheme: theme,
  observedSample: 0,
  observedRender: render,
  component: "Card",
  theme,
  render,
}));
test("DS07 complete public catalog accepts both themes", () => {
  assert.deepEqual(catalogProblems(["Card"], [sample], entries), []);
});
const cases = [
  ["missing public component", ["Card", "Button"], [sample], entries],
  ["missing sample", ["Card"], [], entries],
  ["missing light", ["Card"], [{ ...sample, themes: ["dark"] }], entries],
  ["missing dark", ["Card"], [{ ...sample, themes: ["light"] }], entries],
  [
    "duplicate sample theme",
    ["Card"],
    [{ ...sample, themes: ["light", "light"] }],
    entries,
  ],
  [
    "extra sample theme",
    ["Card"],
    [{ ...sample, themes: ["light", "dark", "other"] }],
    entries,
  ],
  ["absent sample themes", ["Card"], [{ component: "Card", render }], entries],
  [
    "nonpublic sample",
    ["Card"],
    [sample, { ...sample, component: "Hidden" }],
    entries,
  ],
  ["sample render missing", ["Card"], [{ ...sample, render: null }], entries],
  ["light page missing", ["Card"], [sample], [entries[1]]],
  ["dark page missing", ["Card"], [sample], [entries[0]]],
  ["duplicate page", ["Card"], [sample], [...entries, entries[0]]],
  ["duplicate theme", ["Card"], [sample], [entries[0], entries[0]]],
  [
    "nonpublic page",
    ["Card"],
    [sample],
    [...entries, { ...entries[0], component: "Hidden" }],
  ],
  [
    "page render missing",
    ["Card"],
    [sample],
    [{ ...entries[0], render: null }, entries[1]],
  ],
  [
    "unknown theme",
    ["Card"],
    [sample],
    [{ ...entries[0], theme: "system" }, entries[1]],
  ],
];
for (const [name, components, samples, pages] of cases)
  test(`DS07 rejects ${name}`, () =>
    assert.notDeepEqual(catalogProblems(components, samples, pages), []));
test("DS07 theme diagnostics name only the absent dark page", () => {
  assert.deepEqual(catalogProblems(["Card"], [sample], [entries[0]]), [
    "Card: 테마 페이지는 정확히 두 개",
    "Card: dark 페이지 없음 또는 중복",
  ]);
});
for (const field of [
  "title",
  "observedComponent",
  "observedTheme",
  "observedSample",
  "sample",
  "observedRender",
])
  test(`DS07 rejects wrong actual ${field}`, () => {
    const value =
      field === "observedRender"
        ? () => null
        : field === "observedSample" || field === "sample"
          ? 9
          : "wrong";
    assert.notDeepEqual(
      catalogProblems(
        ["Card"],
        [sample],
        [{ ...entries[0], [field]: value }, entries[1]],
      ),
      [],
    );
  });
