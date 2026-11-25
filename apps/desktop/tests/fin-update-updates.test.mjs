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
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
    window.calls = [];
    const ready = {
      current_version: "0.1.0",
      state: { ready: { version: "0.2.0", notes: "- 자동 업데이트를 넣었어요" } },
    };
    window.mount = (path, status, afterCheck) => {
      window.calls = [];
      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings: () => responses.getSettings,
        getUpdateStatus: (args) => {
          window.calls.push(args.request.query);
          return args.request.query === "check-now" ? afterCheck : status;
        },
        restartToUpdate: (args) => {
          window.calls.push(args.request.confirm);
          return status;
        },
      });
      return render(
        h(App, { initialPath: path, queryClient: createAppQueryClient() }),
      );
    };
    window.ready = ready;
    window.off = responses.getUpdateStatus;
    window.upToDate = { current_version: "0.1.0", state: "up-to-date" };
    window.checking = { current_version: "0.1.0", state: "checking" };
    window.failed = {
      current_version: "0.1.0",
      state: { failed: { failure: "check" } },
    };
  `);
});

after(() => bundle.close());

const until = async (w, check, label) =>
  w.waitFor(() => {
    if (!check()) throw new Error(`waiting for ${label}`);
  });

const textOf = (view) => view.container.textContent ?? "";

const buttonNamed = (root, name) =>
  Array.from(root.querySelectorAll("button")).find(
    (node) => node.textContent === name,
  );

test("fin-update: a downloaded update shows a restart band and a bottom sheet with notes", async () => {
  const w = bundle.open();
  const view = w.mount("/", w.ready, w.ready);
  await until(
    w,
    () =>
      textOf(view).includes("새 버전이 준비됐어요. 재시작하면 바로 바뀌어요."),
    "ready band",
  );
  w.fireEvent.click(buttonNamed(view.container, "재시작"));
  await until(
    w,
    () => w.document.body.textContent.includes("새 버전으로 다시 시작할까요?"),
    "restart sheet",
  );
  const body = w.document.body.textContent;
  assert.ok(body.includes("0.2.0"));
  assert.ok(body.includes("자동 업데이트를 넣었어요"));
  assert.deepEqual([...w.calls], ["current"]);
  w.fireEvent.click(buttonNamed(w.document.body, "지금 재시작"));
  await until(w, () => w.calls.includes("restart"), "restart command");
  assert.deepEqual([...w.calls], ["current", "restart"]);
  w.cleanup();
});

test("fin-update: no band appears when the app is up to date or updates are off", async () => {
  for (const status of ["upToDate", "off"]) {
    const w = bundle.open();
    const view = w.mount("/", w[status], w[status]);
    await until(w, () => w.calls.includes("current"), "status query");
    assert.ok(!textOf(view).includes("새 버전이 준비됐어요"));
    assert.equal(buttonNamed(view.container, "재시작"), undefined);
    w.cleanup();
  }
});

test("fin-update: settings show the current version and check on request", async () => {
  const w = bundle.open();
  const view = w.mount("/settings", w.upToDate, w.checking);
  await until(
    w,
    () => textOf(view).includes("최신 버전을 쓰고 있어요."),
    "card",
  );
  assert.ok(textOf(view).includes("앱 업데이트"));
  assert.ok(textOf(view).includes("현재 버전"));
  assert.ok(textOf(view).includes("0.1.0"));
  const check = buttonNamed(view.container, "업데이트 확인");
  assert.equal(check.disabled, false);
  w.fireEvent.click(check);
  await until(
    w,
    () => textOf(view).includes("새 버전을 확인하고 있어요."),
    "checking state",
  );
  assert.ok(w.calls.includes("check-now"));
  assert.equal(buttonNamed(view.container, "업데이트 확인").disabled, true);
  w.cleanup();
});

test("fin-update: failures are explained and the check can be retried; off builds cannot check", async () => {
  const w = bundle.open();
  const view = w.mount("/settings", w.failed, w.failed);
  await until(
    w,
    () => textOf(view).includes("새 버전을 확인하지 못했어요."),
    "failure text",
  );
  assert.equal(buttonNamed(view.container, "업데이트 확인").disabled, false);
  w.cleanup();
  const offView = w.mount("/settings", w.off, w.off);
  await until(
    w,
    () =>
      textOf(offView).includes("이 빌드에서는 자동 업데이트를 쓰지 않아요."),
    "off text",
  );
  assert.equal(buttonNamed(offView.container, "업데이트 확인").disabled, true);
  w.cleanup();
});

test("fin-update: the updater endpoint and key slot live in the Tauri config and the WebView gets no updater permission", () => {
  const conf = JSON.parse(
    readFileSync(join(desktopRoot, "src-tauri/tauri.conf.json"), "utf8"),
  );
  assert.deepEqual(conf.plugins.updater.endpoints, [
    "https://github.com/gihwan-dev/stock-copilot/releases/latest/download/latest.json",
  ]);
  assert.equal(typeof conf.plugins.updater.pubkey, "string");
  assert.ok(conf.plugins.updater.pubkey.length > 0);
  assert.ok(conf.app.security.csp.includes("connect-src ipc:;"));
  const capability = JSON.parse(
    readFileSync(join(desktopRoot, "src-tauri/capabilities/main.json"), "utf8"),
  );
  assert.ok(
    capability.permissions.every(
      (permission) => !String(permission).startsWith("updater:"),
    ),
  );
});
