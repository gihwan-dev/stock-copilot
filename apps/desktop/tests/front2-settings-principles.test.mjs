import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { choose, fakeSource, selectByLabel } from "./front2-fake.mjs";
import { screenDom } from "./screen-dom.mjs";
import { openAdvancedSource } from "./settings-advanced.mjs";

let bundle;

before(async () => {
  bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    ${fakeSource}
  ${openAdvancedSource}
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
    window.mount = (fake) => {
      mockCommands({ appVersion: () => responses.appVersion });
      const client = createAppQueryClient();
      window.openAdvanced();
      const view = render(h(App, { initialPath: "/settings", queryClient: client, settingsClient: fake }));
      return { view, client };
    };
  `);
});

after(() => bundle.close());

const until = async (w, phrase) =>
  w.waitFor(() => {
    if (!w.document.body.textContent.includes(phrase))
      throw new Error(`waiting for ${phrase}`);
  });

const calls = (w, name) => w.fakeCalls.filter((call) => call.name === name);

const principleField = (w) =>
  w.document.body.querySelector('input[aria-label="나의 투자 원칙"]');

const saveButton = (w) =>
  Array.from(w.document.body.querySelectorAll("button")).find(
    (node) =>
      node.textContent === "저장" &&
      principleField(w).compareDocumentPosition(node) &
        w.Node.DOCUMENT_POSITION_FOLLOWING,
  );

test("G-01: defaults show 10% cash and the five-holding concentration warning", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "투자 원칙");
  await w.waitFor(() => {
    if (principleField(w) === null) throw new Error("waiting for the form");
  });
  assert.equal(selectByLabel(w, "현금 최소 비중").value, "10");
  assert.equal(selectByLabel(w, "쏠림 경고 기준").value, "5");
  assert.ok(
    w.document.body.textContent.includes("유효 종목 수 5개 미만이면 경고"),
  );
  w.cleanup();
});

test("G-01: saving sends the free text, cash floor and concentration threshold together", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await w.waitFor(() => {
    if (principleField(w) === null) throw new Error("waiting for the form");
  });
  w.fireEvent.change(principleField(w), {
    target: { value: "실적이 꺾인 종목은 사지 않는다" },
  });
  choose(w, "현금 최소 비중", "20");
  choose(w, "쏠림 경고 기준", "8");
  assert.equal(calls(w, "setPrinciples").length, 0);
  w.fireEvent.click(saveButton(w));
  await until(w, "저장했어요");
  assert.equal(calls(w, "setPrinciples").length, 1);
  assert.deepEqual(JSON.parse(calls(w, "setPrinciples")[0].args)[0], {
    text: "실적이 꺾인 종목은 사지 않는다",
    cash_min_percent: "20",
    concentration_min_count: "8",
  });
  w.cleanup();
});

test("G-01: an over-long principle is refused before reaching the core", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await w.waitFor(() => {
    if (principleField(w) === null) throw new Error("waiting for the form");
  });
  w.fireEvent.change(principleField(w), {
    target: { value: "가".repeat(4001) },
  });
  w.fireEvent.click(saveButton(w));
  await until(w, "4000자 안으로");
  assert.equal(calls(w, "setPrinciples").length, 0);
  w.cleanup();
});

test("G-01: a failed save and a failed load each show one sentence", async () => {
  const w = bundle.open();
  w.mount(w.makeFake({ setPrinciples: () => w.failedReply("retryable") }));
  await w.waitFor(() => {
    if (principleField(w) === null) throw new Error("waiting for the form");
  });
  w.fireEvent.click(saveButton(w));
  await until(w, "저장하지 못했어요");
  w.cleanup();
  const second = bundle.open();
  second.mount(
    second.makeFake({ principles: () => second.failedReply("bug") }),
  );
  await until(second, "투자 원칙을 불러오지 못했어요");
  second.cleanup();
});
