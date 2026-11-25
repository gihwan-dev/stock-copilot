import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { choose, fakeSource, okReply, selectByLabel } from "./front2-fake.mjs";
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
    window.mount = (fake, path = "/settings") => {
      mockCommands({ appVersion: () => responses.appVersion });
      const client = createAppQueryClient();
      window.openAdvanced();
      const view = render(h(App, { initialPath: path, queryClient: client, settingsClient: fake }));
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

const button = (w, label) =>
  Array.from(w.document.body.querySelectorAll("button")).find(
    (node) => node.textContent === label,
  );

const calls = (w, name) => w.fakeCalls.filter((call) => call.name === name);

test("U-22: with several accounts the system section offers a choice and switching invalidates the other queries", async () => {
  const w = bundle.open();
  const { client } = w.mount(w.makeFake());
  await until(w, "쓸 계좌");
  assert.ok(selectByLabel(w, "쓸 계좌"));
  const number = w.document.body.querySelector('input[aria-label="계좌 번호"]');
  assert.equal(number.value, "123-45-0001");
  client.setQueryData(["portfolio"], { kind: "ok", value: {} });
  choose(w, "쓸 계좌", "acct-2");
  await w.waitFor(() => {
    if (calls(w, "selectAccount").length !== 1)
      throw new Error("waiting for selectAccount");
  });
  assert.equal(JSON.parse(calls(w, "selectAccount")[0].args)[0], "acct-2");
  await w.waitFor(() => {
    if (!client.getQueryState(["portfolio"]).isInvalidated)
      throw new Error("waiting for invalidation");
  });
  await w.waitFor(() => {
    const shown = w.document.body.querySelector(
      'input[aria-label="계좌 번호"]',
    );
    if (shown.value !== "123-45-0002") throw new Error("waiting for number");
  });
  assert.equal(
    client.getQueryState(["account-selection"]).isInvalidated,
    false,
  );
  w.cleanup();
});

test("U-22: a single account shows no account choice, and first run never asks for one", async () => {
  const w = bundle.open();
  const single = w.makeFake({
    accountSelection: () =>
      okReply(w, {
        selected: "acct-1",
        accounts: [{ id: "acct-1", number: "123-45-0001" }],
      }),
  });
  w.mount(single);
  await until(w, "토스 자료 갱신");
  assert.equal(selectByLabel(w, "쓸 계좌"), null);
  w.cleanup();
  const first = bundle.open();
  first.mount(
    first.makeFake({
      checkTossConnection: () => first.connectionReply("needs-keys"),
    }),
    "/",
  );
  await until(first, "client_id와 secret");
  assert.ok(!first.document.body.textContent.includes("쓸 계좌"));
  first.cleanup();
});

test("U-08: the sync schedule is described in plain sentences", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "토스 자료 갱신");
  const body = w.document.body.textContent;
  assert.ok(body.includes("5분마다"));
  assert.ok(body.includes("주문 직후"));
  assert.ok(body.includes("30분마다 최근 14일"));
  assert.ok(body.includes("하루 한 번 전체 기간"));
  assert.ok(body.includes("토스 장 운영 정보"));
  w.cleanup();
});

test("U-15: backup and export report their outcome in one sentence", async () => {
  const w = bundle.open();
  const fake = w.makeFake({
    exportData: () => okReply(w, { status: "cancelled" }),
  });
  w.mount(fake);
  await until(w, "지금 백업");
  w.fireEvent.click(button(w, "지금 백업"));
  await until(w, "백업했어요");
  w.fireEvent.click(button(w, "내보내기"));
  await until(w, "내보내기를 취소했어요");
  assert.equal(calls(w, "backupNow").length, 1);
  assert.equal(calls(w, "exportData").length, 1);
  w.cleanup();
});

test("U-15: delete all asks first, cancelling sends nothing", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await until(w, "전체 삭제");
  w.fireEvent.click(button(w, "전체 삭제"));
  await until(w, "모든 데이터를 지울까요?");
  w.fireEvent.click(button(w, "취소"));
  await w.waitFor(() => {
    if (w.document.body.textContent.includes("모든 데이터를 지울까요?"))
      throw new Error("waiting for the dialog to close");
  });
  assert.equal(calls(w, "deleteAllData").length, 0);
  w.cleanup();
});

test("U-15: confirmed delete invalidates every stored query and returns to the key screen", async () => {
  const w = bundle.open();
  const fake = w.makeFake({
    checkTossConnection: w.sequence(["connected", "needs-keys"]),
  });
  const { client } = w.mount(fake);
  await until(w, "전체 삭제");
  client.setQueryData(["transactions"], { kind: "ok", value: {} });
  w.fireEvent.click(button(w, "전체 삭제"));
  await until(w, "모든 데이터를 지울까요?");
  w.fireEvent.click(button(w, "모두 지우기"));
  await until(w, "client_id와 secret을 붙여 넣으세요");
  assert.equal(calls(w, "deleteAllData").length, 1);
  assert.equal(client.getQueryState(["transactions"]).isInvalidated, true);
  assert.equal(calls(w, "checkTossConnection").length, 2);
  w.cleanup();
});

test("U-15: a failed backup says so", async () => {
  const w = bundle.open();
  w.mount(w.makeFake({ backupNow: () => w.failedReply("retryable") }));
  await until(w, "지금 백업");
  w.fireEvent.click(button(w, "지금 백업"));
  await until(w, "하지 못했어요");
  w.cleanup();
});
