import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

let bundle;

before(async () => {
  bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
    import { App, CoreApp } from APP_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
    window.mountCore = (settings = responses.getSettings) => {
      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings: () => settings,
      });
      return render(h(CoreApp));
    };
    window.mountPlain = () => {
      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings: () => responses.getSettings,
      });
      return render(h(App));
    };
    window.connectedSettings = () => ({
      ...responses.getSettings,
      toss_connection: "connected",
    });
  `);
});

after(() => bundle.close());

const until = async (w, view, phrase) =>
  w.waitFor(() => {
    if (!view.container.textContent.includes(phrase))
      throw new Error(`waiting for ${phrase}`);
  });

const buttons = (view) =>
  Array.from(view.container.querySelectorAll("button")).map(
    (node) => node.textContent,
  );

test("fin-ux: the shipped assembly without Toss keys opens on the key screen", async () => {
  const w = bundle.open();
  const view = w.mountCore();
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  assert.ok(view.container.querySelector("h1"));
  assert.equal(
    view.container.querySelector("h1").textContent,
    "토스증권 키를 붙여 넣어 주세요",
  );
  assert.ok(view.container.querySelector('input[aria-label="client_id"]'));
  assert.ok(view.container.querySelector('input[aria-label="secret"]'));
  assert.ok(
    view.container.querySelector('ol[aria-label] li[aria-current="step"]'),
  );
  assert.deepEqual(buttons(view), [
    "붙여넣기",
    "붙여넣기",
    "연결하기",
    "나중에 할게요",
  ]);
  assert.ok(!view.container.textContent.includes("앱이 준비됐어요."));
  w.cleanup();
});

test("fin-ux: choosing later in the shipped assembly reaches the main screen without keys", async () => {
  const w = bundle.open();
  const view = w.mountCore();
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  const later = Array.from(view.container.querySelectorAll("button")).find(
    (node) => node.textContent === "나중에 할게요",
  );
  w.fireEvent.click(later);
  await until(w, view, "앱이 준비됐어요.");
  assert.equal(
    view.container.querySelector('input[aria-label="client_id"]'),
    null,
  );
  w.cleanup();
});

test("fin-ux: the shipped assembly without keys cannot pretend the keys were stored", async () => {
  const w = bundle.open();
  const view = w.mountCore();
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  w.fireEvent.change(
    view.container.querySelector('input[aria-label="client_id"]'),
    {
      target: { value: "fake-client-id" },
    },
  );
  w.fireEvent.change(
    view.container.querySelector('input[aria-label="secret"]'),
    {
      target: { value: "fake-client-secret" },
    },
  );
  const connect = Array.from(view.container.querySelectorAll("button")).find(
    (node) => node.textContent === "연결하기",
  );
  w.fireEvent.click(connect);
  await until(w, view, "토스에 연결하지 못했어요");
  assert.ok(!view.container.textContent.includes("앱이 준비됐어요."));
  assert.ok(!view.container.innerHTML.includes("fake-client-secret"));
  w.cleanup();
});

test("fin-ux: the shipped assembly with a connected core goes straight to the main screen", async () => {
  const w = bundle.open();
  const view = w.mountCore(w.connectedSettings());
  await until(w, view, "앱이 준비됐어요.");
  assert.equal(
    view.container.querySelector('input[aria-label="client_id"]'),
    null,
  );
  w.cleanup();
});

const byLabel = (view, label) =>
  view.container.querySelector(`button[aria-label="${label}"]`);

test("fin-ux: the Toss row in settings reopens the key screen and later returns to settings", async () => {
  const w = bundle.open();
  w.location.hash = "#/settings";
  const view = w.mountCore(w.connectedSettings());
  await until(w, view, "기본 설정");
  assert.deepEqual(
    Array.from(
      view.container.querySelectorAll("h3"),
      (node) => node.textContent,
    ),
    ["토스 연결", "투자 성향", "주문 켜기"],
  );
  assert.ok(
    view.container.textContent.includes("토스증권 계좌가 연결되어 있어요."),
  );
  w.fireEvent.click(byLabel(view, "키 다시 붙여 넣기"));
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  const later = Array.from(view.container.querySelectorAll("button")).find(
    (node) => node.textContent === "나중에 할게요",
  );
  w.fireEvent.click(later);
  await until(w, view, "기본 설정");
  assert.equal(
    view.container.querySelector('input[aria-label="client_id"]'),
    null,
  );
  w.cleanup();
});

test("fin-ux: the investor profile row folds its details", async () => {
  const w = bundle.open();
  w.location.hash = "#/settings";
  const view = w.mountCore(w.connectedSettings());
  await until(w, view, "기본 설정");
  const fold = byLabel(view, "투자 성향 자세히 보기");
  assert.equal(fold.getAttribute("aria-expanded"), "false");
  assert.ok(!view.container.textContent.includes("위험 성향"));
  w.fireEvent.click(fold);
  await until(w, view, "위험 성향");
  assert.equal(
    byLabel(view, "투자 성향 자세히 보기").getAttribute("aria-expanded"),
    "true",
  );
  assert.ok(view.container.textContent.includes("종목 상한"));
  w.fireEvent.click(byLabel(view, "투자 성향 자세히 보기"));
  await w.waitFor(() => {
    if (view.container.textContent.includes("위험 성향"))
      throw new Error("waiting for fold");
  });
  w.cleanup();
});

test("fin-ux: without the setup gate the Toss row has no reopen link", async () => {
  const w = bundle.open();
  w.location.hash = "#/settings";
  const view = w.mountPlain();
  await until(w, view, "기본 설정");
  assert.equal(byLabel(view, "키 다시 붙여 넣기"), null);
  w.cleanup();
});

test("fin-ux: the real entry mounts the shipped assembly", () => {
  const main = readFileSync(join(desktopRoot, "src/app/main.tsx"), "utf8");
  assert.match(main, /<CoreApp \/>/u);
  assert.doesNotMatch(main, /<App[ />]/u);
});
