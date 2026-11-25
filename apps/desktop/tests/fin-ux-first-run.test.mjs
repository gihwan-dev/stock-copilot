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
    import { CoreApp } from APP_ENTRY;
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

test("fin-ux: the real entry mounts the shipped assembly", () => {
  const main = readFileSync(join(desktopRoot, "src/app/main.tsx"), "utf8");
  assert.match(main, /<CoreApp \/>/u);
  assert.doesNotMatch(main, /<App[ />]/u);
});
