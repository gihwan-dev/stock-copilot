import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { fakeSource } from "./front2-fake.mjs";
import { screenDom } from "./screen-dom.mjs";

let bundle;

before(async () => {
  bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, waitFor, cleanup, act } from "@testing-library/react";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    ${fakeSource}
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
    window.act = act;
    window.mount = (fake) => {
      mockCommands({ appVersion: () => responses.appVersion });
      const client = createAppQueryClient();
      const view = render(h(App, { initialPath: "/", queryClient: client, settingsClient: fake }));
      return { view, client };
    };
  `);
});

after(() => bundle.close());

const until = async (w, view, phrase) =>
  w.waitFor(() => {
    if (!view.container.textContent.includes(phrase))
      throw new Error(`waiting for ${phrase}`);
  });

const button = (view, label) =>
  Array.from(view.container.querySelectorAll("button")).find(
    (node) => node.textContent === label,
  );

const countCalls = (w, name) =>
  w.fakeCalls.filter((call) => call.name === name).length;

const captureIntervals = (w) => {
  const live = new Map();
  let next = 1;
  const original = w.setInterval.bind(w);
  const clear = w.clearInterval.bind(w);
  w.setInterval = (callback, delay, ...rest) => {
    if (delay !== 60_000) return original(callback, delay, ...rest);
    const id = 100_000 + next;
    next += 1;
    live.set(id, callback);
    return id;
  };
  w.clearInterval = (id) => {
    if (live.has(id)) live.delete(id);
    else clear(id);
  };
  return live;
};

const tick = async (w, live) => {
  await w.act(async () => {
    for (const callback of [...live.values()]) callback();
  });
};

test("U-07: the IP screen shows the current public IP, the WTS location and copies it", async () => {
  const w = bundle.open();
  const fake = w.makeFake({
    checkTossConnection: () => w.connectionReply("ip-not-allowed"),
  });
  const { view } = w.mount(fake);
  await until(w, view, "허용 IP 관리에 아래 IP를 추가하세요");
  await w.waitFor(() => {
    const input = view.container.querySelector('input[aria-label="현재 IP"]');
    if (input?.value !== "203.0.113.7") throw new Error("waiting for ip");
  });
  assert.ok(view.container.textContent.includes("WTS > 설정 > Open API"));
  w.fireEvent.click(button(view, "IP 복사"));
  await until(w, view, "복사했어요");
  const copies = w.fakeCalls.filter((call) => call.name === "copyText");
  assert.equal(copies.length, 1);
  assert.equal(JSON.parse(copies[0].args)[0], "203.0.113.7");
  w.cleanup();
});

test("U-07: the public IP is never asked for outside the IP screen", async () => {
  const w = bundle.open();
  const fake = w.makeFake({
    checkTossConnection: () => w.connectionReply("needs-keys"),
  });
  const { view } = w.mount(fake);
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  assert.equal(countCalls(w, "publicIp"), 0);
  w.cleanup();
  const second = bundle.open();
  const passing = second.makeFake();
  const mounted = second.mount(passing);
  await until(second, mounted.view, "앱이 준비됐어요.");
  assert.equal(
    second.fakeCalls.filter((call) => call.name === "publicIp").length,
    0,
  );
  second.cleanup();
});

test("U-07: while the IP screen is open the app retries once a minute and moves on when it passes", async () => {
  const w = bundle.open();
  const live = captureIntervals(w);
  const fake = w.makeFake({
    checkTossConnection: w.sequence([
      "ip-not-allowed",
      "ip-not-allowed",
      "connected",
    ]),
  });
  const { view } = w.mount(fake);
  await until(w, view, "허용 IP 관리에 아래 IP를 추가하세요");
  await w.waitFor(() => {
    if (live.size !== 1) throw new Error("waiting for the one-minute retry");
  });
  assert.equal(countCalls(w, "checkTossConnection"), 1);
  await tick(w, live);
  await w.waitFor(() => {
    if (countCalls(w, "checkTossConnection") !== 2)
      throw new Error("waiting for the second check");
  });
  assert.ok(view.container.textContent.includes("허용 IP 등록"));
  await tick(w, live);
  await until(w, view, "앱이 준비됐어요.");
  assert.equal(countCalls(w, "checkTossConnection"), 3);
  await w.waitFor(() => {
    if (live.size !== 0) throw new Error("waiting for the retry to stop");
  });
  assert.equal(countCalls(w, "publicIp"), 1);
  w.cleanup();
});

test("U-07: no retry timer runs on the key screen", async () => {
  const w = bundle.open();
  const live = captureIntervals(w);
  const fake = w.makeFake({
    checkTossConnection: () => w.connectionReply("needs-keys"),
  });
  const { view } = w.mount(fake);
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  assert.equal(live.size, 0);
  w.cleanup();
});

test("U-07: a failed IP lookup says so in one sentence and keeps retrying the connection", async () => {
  const w = bundle.open();
  const live = captureIntervals(w);
  const fake = w.makeFake({
    checkTossConnection: () => w.connectionReply("ip-not-allowed"),
    publicIp: () => w.failedReply("retryable"),
  });
  const { view } = w.mount(fake);
  await until(w, view, "현재 IP를 확인하지 못했어요");
  await w.waitFor(() => {
    if (live.size !== 1) throw new Error("waiting for the one-minute retry");
  });
  w.cleanup();
});
