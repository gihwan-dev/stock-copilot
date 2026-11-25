import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { screenDom } from "./screen-dom.mjs";

let bundle;

before(async () => {
  bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, waitFor, cleanup, act } from "@testing-library/react";
    import { CoreApp } from APP_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
    window.act = act;
    window.core = { checks: [], submits: [], lookups: 0, copies: [] };
    window.mountCore = (script) => {
      window.core = { checks: [], submits: [], lookups: 0, copies: [] };
      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings: () => responses.getSettings,
        checkTossConnection: ({ request }) => {
          window.core.checks.push(request);
          return { state: script.checks.shift() ?? "connected" };
        },
        submitTossKeys: ({ request }) => {
          window.core.submits.push(request);
          return { state: script.submit };
        },
        getPublicIp: () => {
          window.core.lookups += 1;
          return responses.getPublicIp;
        },
        copyText: ({ request }) => {
          window.core.copies.push(request);
          return responses.copyText;
        },
      });
      return render(h(CoreApp));
    };
  `);
});

after(() => bundle.close());

const until = async (w, view, phrase) =>
  w.waitFor(() => {
    if (!view.container.textContent.includes(phrase))
      throw new Error(`waiting for ${phrase}`);
  });

const captureMinuteTimer = (w) => {
  const live = new Map();
  const original = w.setInterval.bind(w);
  const clear = w.clearInterval.bind(w);
  w.setInterval = (callback, delay, ...rest) => {
    if (delay !== 60_000) return original(callback, delay, ...rest);
    const id = 900_000 + live.size;
    live.set(id, callback);
    return id;
  };
  w.clearInterval = (id) => (live.has(id) ? live.delete(id) : clear(id));
  return live;
};

const pasteKeys = (w, view) => {
  w.fireEvent.change(
    view.container.querySelector('input[aria-label="client_id"]'),
    {
      target: { value: "  fake-client-id\n" },
    },
  );
  w.fireEvent.change(
    view.container.querySelector('input[aria-label="secret"]'),
    {
      target: { value: "fake-client-secret" },
    },
  );
};

const press = (w, view, label, operations) => {
  const target = Array.from(view.container.querySelectorAll("button")).find(
    (node) => node.textContent === label,
  );
  assert.ok(target, label);
  operations.push(label);
  w.fireEvent.click(target);
};

test("U-28 core: pasted keys go to the core, the IP screen copies the core's public IP and the minute retry reaches the main screen", async () => {
  const w = bundle.open();
  const live = captureMinuteTimer(w);
  const operations = [];
  const view = w.mountCore({
    checks: ["needs-keys", "connected"],
    submit: "ip-not-allowed",
  });
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  pasteKeys(w, view);
  press(w, view, "연결하기", operations);
  await until(w, view, "허용 IP 관리에 아래 IP를 추가하세요");
  assert.deepEqual(JSON.parse(JSON.stringify(w.core.submits)), [
    { client_id: "fake-client-id", secret: "fake-client-secret" },
  ]);
  await w.waitFor(() => {
    const ip = view.container.querySelector('input[aria-label="현재 IP"]');
    if (ip?.value !== "203.0.113.10") throw new Error("waiting for ip");
  });
  press(w, view, "IP 복사", operations);
  await until(w, view, "복사했어요");
  assert.deepEqual(JSON.parse(JSON.stringify(w.core.copies)), [
    { text: "203.0.113.10" },
  ]);
  assert.equal(w.core.lookups, 1);
  await w.waitFor(() => {
    if (live.size !== 1) throw new Error("waiting for the retry timer");
  });
  await w.act(async () => {
    for (const callback of [...live.values()]) callback();
  });
  await until(w, view, "앱이 준비됐어요.");
  assert.deepEqual(operations, ["연결하기", "IP 복사"]);
  assert.deepEqual(JSON.parse(JSON.stringify(w.core.checks)), [
    { query: "current" },
    { query: "current" },
  ]);
  assert.ok(!view.container.innerHTML.includes("fake-client-secret"));
  w.cleanup();
});

test("U-28 core: keys that connect at once skip the IP screen and never ask for the public IP", async () => {
  const w = bundle.open();
  const operations = [];
  const view = w.mountCore({ checks: ["needs-keys"], submit: "connected" });
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  pasteKeys(w, view);
  press(w, view, "연결하기", operations);
  await until(w, view, "앱이 준비됐어요.");
  assert.deepEqual(operations, ["연결하기"]);
  assert.equal(w.core.lookups, 0);
  assert.ok(!view.container.innerHTML.includes("fake-client-secret"));
  w.cleanup();
});

test("U-28 core: a key the core rejects keeps the key screen with the problem shown", async () => {
  const w = bundle.open();
  const operations = [];
  const view = w.mountCore({ checks: ["needs-keys"], submit: "invalid-key" });
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  pasteKeys(w, view);
  press(w, view, "연결하기", operations);
  await w.waitFor(() => {
    if (w.core.submits.length !== 1) throw new Error("waiting for submit");
  });
  await new Promise((resolve) => setTimeout(resolve, 50));
  assert.ok(view.container.querySelector('input[aria-label="client_id"]'));
  assert.ok(!view.container.textContent.includes("앱이 준비됐어요."));
  assert.equal(w.core.lookups, 0);
  w.cleanup();
});
