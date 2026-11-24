import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { fakeSource } from "./front2-fake.mjs";
import { desktopRoot } from "./paths.mjs";
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
    window.mount = (fake, path = "/") => {
      mockCommands({ appVersion: () => responses.appVersion });
      const client = createAppQueryClient();
      const view = render(h(App, { initialPath: path, queryClient: client, settingsClient: fake }));
      return { view, client };
    };
  `);
});

after(() => bundle.close());

const terminalWords =
  /터미널|명령어|명령을 실행|Terminal|terminal|shell|npm |brew |sudo|claude login|\$ /u;

const until = async (w, phrase) =>
  w.waitFor(() => {
    if (!w.document.body.textContent.includes(phrase))
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

test("U-28: pasting the keys and registering the IP are the only two operations before the main screen", async () => {
  const w = bundle.open();
  const live = captureMinuteTimer(w);
  const screens = [];
  const operations = [];
  const press = (label) => {
    const target = Array.from(w.document.body.querySelectorAll("button")).find(
      (node) => node.textContent === label,
    );
    assert.ok(target, label);
    operations.push(label);
    w.fireEvent.click(target);
  };
  const fake = w.makeFake({
    checkTossConnection: w.sequence(["needs-keys", "connected"]),
    submitTossKeys: () => w.connectionReply("ip-not-allowed"),
  });
  w.mount(fake);
  await until(w, "client_id와 secret을 붙여 넣으세요");
  screens.push(w.document.body.textContent);
  w.fireEvent.change(
    w.document.querySelector('input[aria-label="client_id"]'),
    {
      target: { value: "fake-client-id" },
    },
  );
  w.fireEvent.change(w.document.querySelector('input[aria-label="secret"]'), {
    target: { value: "fake-client-secret" },
  });
  press("연결하기");
  await until(w, "허용 IP 관리에 아래 IP를 추가하세요");
  await w.waitFor(() => {
    const ip = w.document.querySelector('input[aria-label="현재 IP"]');
    if (ip?.value !== "203.0.113.7") throw new Error("waiting for ip");
  });
  screens.push(w.document.body.textContent);
  press("IP 복사");
  await until(w, "복사했어요");
  await w.waitFor(() => {
    if (live.size !== 1) throw new Error("waiting for the retry timer");
  });
  await w.act(async () => {
    for (const callback of [...live.values()]) callback();
  });
  await until(w, "앱이 준비됐어요.");
  screens.push(w.document.body.textContent);
  assert.deepEqual(operations, ["연결하기", "IP 복사"]);
  assert.ok(!w.document.body.textContent.includes("쓸 계좌"));
  for (const text of screens) assert.ok(!terminalWords.test(text), text);
  w.cleanup();
});

test("U-28: the settings screens never ask for a terminal command", async () => {
  const w = bundle.open();
  w.mount(w.makeFake(), "/settings");
  await until(w, "작업별 모델");
  await until(w, "수집 소스");
  await until(w, "나의 투자 원칙");
  await until(w, "토스 자료 갱신");
  assert.ok(!terminalWords.test(w.document.body.textContent));
  w.cleanup();
});

const messageFiles = (folder) =>
  readdirSync(folder).flatMap((name) => {
    const path = join(folder, name);
    if (statSync(path).isDirectory()) return messageFiles(path);
    return name === "messages.ts" ? [path] : [];
  });

test("U-28: no screen text in the app asks for a terminal command", () => {
  const files = messageFiles(join(desktopRoot, "src"));
  assert.ok(files.length >= 6);
  for (const file of files) {
    const phrases = [
      ...readFileSync(file, "utf8").matchAll(/ko:\s*"((?:[^"\\]|\\.)*)"/gu),
    ].map((match) => match[1]);
    assert.ok(phrases.length > 0, file);
    for (const phrase of phrases)
      assert.ok(!terminalWords.test(phrase), `${file}: ${phrase}`);
  }
});
