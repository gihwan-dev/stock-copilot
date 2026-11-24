import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { fakeSource } from "./front2-fake.mjs";
import { screenDom } from "./screen-dom.mjs";

const secretId = "ci-test-client-id-7f3a";
const secretValue = "ci-test-secret-9b2c";

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
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
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

const field = (view, label) =>
  view.container.querySelector(`input[aria-label="${label}"]`);

const button = (view, label) =>
  Array.from(view.container.querySelectorAll("button")).find(
    (node) => node.textContent === label,
  );

const typeKeys = (w, view, id, secret) => {
  w.fireEvent.change(field(view, "client_id"), { target: { value: id } });
  w.fireEvent.change(field(view, "secret"), { target: { value: secret } });
};

const captureConsole = (w) => {
  const lines = [];
  for (const level of ["log", "info", "warn", "error", "debug"]) {
    const original = w.console[level];
    w.console[level] = (...args) => {
      lines.push(args.map(String).join(" "));
      return original;
    };
  }
  return lines;
};

const cacheText = (client) =>
  JSON.stringify([
    client
      .getQueryCache()
      .getAll()
      .map((q) => q.state),
    client
      .getMutationCache()
      .getAll()
      .map((m) => m.state),
  ]);

test("U-06: pasted keys go to the core once, then vanish from the screen, caches and logs", async () => {
  const w = bundle.open();
  const logs = captureConsole(w);
  const fake = w.makeFake({
    checkTossConnection: () => w.connectionReply("needs-keys"),
  });
  const { view, client } = w.mount(fake);
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  assert.equal(field(view, "secret").type, "password");
  typeKeys(w, view, secretId, secretValue);
  w.fireEvent.click(button(view, "연결하기"));
  await until(w, view, "앱이 준비됐어요.");
  const submits = w.fakeCalls.filter((call) => call.name === "submitTossKeys");
  assert.equal(submits.length, 1);
  assert.deepEqual(JSON.parse(submits[0].args)[0], {
    clientId: secretId,
    secret: secretValue,
  });
  assert.ok(!view.container.innerHTML.includes(secretValue));
  assert.ok(!view.container.innerHTML.includes(secretId));
  assert.ok(!cacheText(client).includes(secretValue));
  assert.ok(!cacheText(client).includes(secretId));
  assert.ok(logs.every((line) => !line.includes(secretValue)));
  w.cleanup();
});

test("U-06: a rejected key shows the fix sentence and clears both fields", async () => {
  const w = bundle.open();
  const fake = w.makeFake({
    checkTossConnection: () => w.connectionReply("needs-keys"),
    submitTossKeys: () => w.connectionReply("invalid-key"),
  });
  const { view, client } = w.mount(fake);
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  typeKeys(w, view, secretId, secretValue);
  w.fireEvent.click(button(view, "연결하기"));
  await until(w, view, "토스 키가 맞지 않습니다");
  assert.equal(field(view, "client_id").value, "");
  assert.equal(field(view, "secret").value, "");
  assert.ok(!cacheText(client).includes(secretValue));
  w.cleanup();
});

test("U-06: blank or spaced keys are refused before reaching the core", async () => {
  const w = bundle.open();
  const fake = w.makeFake({
    checkTossConnection: () => w.connectionReply("needs-keys"),
  });
  const { view } = w.mount(fake);
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  typeKeys(w, view, "has space", "");
  w.fireEvent.click(button(view, "연결하기"));
  await until(w, view, "키 형식이 맞지 않아요");
  assert.equal(
    w.fakeCalls.filter((call) => call.name === "submitTossKeys").length,
    0,
  );
  w.cleanup();
});

test("U-06: a failed connection call keeps the key screen with a retry sentence", async () => {
  const w = bundle.open();
  const fake = w.makeFake({
    checkTossConnection: () => w.connectionReply("needs-keys"),
    submitTossKeys: () => w.failedReply("retryable"),
  });
  const { view } = w.mount(fake);
  await until(w, view, "client_id와 secret을 붙여 넣으세요");
  typeKeys(w, view, secretId, secretValue);
  w.fireEvent.click(button(view, "연결하기"));
  await until(w, view, "토스에 연결하지 못했어요");
  assert.ok(field(view, "client_id"));
  w.cleanup();
});

test("U-06: without a core answer the app opens the main screen and asks nothing", async () => {
  const w = bundle.open();
  const fake = w.makeFake({
    checkTossConnection: () => w.failedReply("no-data"),
  });
  const { view } = w.mount(fake);
  await until(w, view, "앱이 준비됐어요.");
  assert.equal(field(view, "client_id"), null);
  w.cleanup();
});
