import { strict as assert } from "node:assert";
import { test } from "node:test";
import { stressProblems } from "../../../harness/ds-catalog/rules.mjs";
import { loadCatalog } from "../../../harness/ds-catalog/load.mjs";

test("each public component must have its own stress story", () => {
  const components = ["Text", "Button", "Money"];
  const samples = [
    { component: "Text", state: "stress", sample: 1 },
    { component: "Button", state: "normal", sample: 1 },
    { component: "Money", state: "stress", sample: 2 },
  ];
  assert.deepEqual(stressProblems(components, samples), [
    "Button: 부하 이야기 없음",
  ]);
  assert.deepEqual(
    stressProblems(components, [
      ...samples,
      { component: "Button", state: "stress", sample: 3 },
    ]),
    [],
  );
  assert.deepEqual(stressProblems(["Text", "Missing"], samples), [
    "Missing: 부하 이야기 없음",
  ]);
});

test("the actual catalog provides stress stories for all public components", async () => {
  const catalog = await loadCatalog();
  assert.deepEqual(
    stressProblems(Object.keys(catalog.components), catalog.samples),
    [],
  );
});

test("the additional ChatInput stress story starts with protected stress content", async () => {
  const catalog = await loadCatalog();
  const chatStories = catalog.samples.filter(
    (story) => story.component === "ChatInput" && story.state === "stress",
  );
  const draft = chatStories.at(-1)?.render().props.initialDraft;
  assert.equal(typeof draft, "string");
  assert.ok(draft.includes("대한미래첨단산업글로벌에너지"));
});

test("stress stories exercise long digits and an empty value", async () => {
  const catalog = await loadCatalog();
  const input = catalog.samples.find(
    (story) => story.component === "TextField" && story.state === "stress",
  );
  const empty = catalog.samples.filter(
    (story) => story.component === "EmptyState" && story.state === "stress",
  );
  assert.equal(
    input?.render().props.defaultValue,
    "12345678901234567890123456789012345678901234567890",
  );
  assert.ok(empty.some((story) => story.render().props.title === ""));
});
