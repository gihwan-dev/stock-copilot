import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { fakeSource } from "./front2-fake.mjs";
import { screenDom } from "./screen-dom.mjs";

let bundle;

before(async () => {
  bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, waitFor, cleanup } from "@testing-library/react";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands, events } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    ${fakeSource}
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.emitCore = (payload) => events.uiEvent.emit(payload);
    window.mount = (fake) => {
      mockCommands({ appVersion: () => responses.appVersion });
      const client = createAppQueryClient();
      return render(h(App, { initialPath: "/settings", queryClient: client, settingsClient: fake }));
    };
  `);
});

after(() => bundle.close());

const count = (w, name) =>
  w.fakeCalls.filter((call) => call.name === name).length;

const settle = (w) => new w.Promise((resolve) => w.setTimeout(resolve, 80));

const ready = async (w) => {
  await w.waitFor(() => {
    if (count(w, "principles") < 1 || count(w, "newsSettings") < 1)
      throw new Error("waiting for the first reads");
  });
  await settle(w);
};

test("U-05: SettingsChanged re-reads every advanced settings query", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await ready(w);
  const before = [
    "principles",
    "newsSettings",
    "aiSettings",
    "scheduledJobs",
  ].map((name) => count(w, name));
  await w.emitCore("SettingsChanged");
  await w.waitFor(() => {
    if (count(w, "principles") <= before[0]) throw new Error("waiting");
  });
  const afterCounts = [
    "principles",
    "newsSettings",
    "aiSettings",
    "scheduledJobs",
  ].map((name) => count(w, name));
  afterCounts.forEach((value, index) => {
    assert.ok(value > before[index], `query ${index} was not re-read`);
  });
  w.cleanup();
});

test("U-05: activity events and unknown payloads leave settings alone", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await ready(w);
  const principles = count(w, "principles");
  const news = count(w, "newsSettings");
  await w.emitCore({ NotAnEvent: "x" });
  await w.emitCore("NotAnEvent");
  await w.emitCore({ PriceMoved: "005930" });
  await settle(w);
  assert.equal(count(w, "principles"), principles);
  assert.equal(count(w, "newsSettings"), news);
  w.cleanup();
});

test("U-05: StoredDataChanged re-reads everything and unmounting stops listening", async () => {
  const w = bundle.open();
  w.mount(w.makeFake());
  await ready(w);
  const accounts = count(w, "accountSelection");
  await w.emitCore("StoredDataChanged");
  await w.waitFor(() => {
    if (count(w, "accountSelection") <= accounts) throw new Error("waiting");
  });
  w.cleanup();
  await settle(w);
  const principles = count(w, "principles");
  await w.emitCore("SettingsChanged");
  await settle(w);
  assert.equal(count(w, "principles"), principles);
});
