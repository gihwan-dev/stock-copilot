import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { fakeSource } from "./front2-fake.mjs";
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
    ${fakeSource}
    window.cleanup = cleanup;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
    window.mount = (fake, settings) => {
      mockCommands({
        appVersion: () => responses.appVersion,
        ...(settings === "ready" ? { getSettings: () => responses.getSettings } : {}),
      });
      return render(h(App, { initialPath: "/settings", queryClient: createAppQueryClient(), settingsClient: fake }));
    };
  `);
});

after(() => bundle.close());

const until = async (w, view, phrase) =>
  w.waitFor(() => {
    if (!view.container.textContent.includes(phrase))
      throw new Error(`waiting for ${phrase}`);
  });

const editorPhrases = [
  "작업별 모델",
  "수집 소스",
  "나의 투자 원칙",
  "투자 성향과 주문 설정 바꾸기",
];

for (const settings of ["ready", "failed"]) {
  test(`fin-core: the editing area stays inside the closed 고급 fold until opened (${settings} settings)`, async () => {
    const w = bundle.open();
    const view = w.mount(w.makeFake(), settings);
    const trigger = await w.waitFor(() => {
      const found = Array.from(view.container.querySelectorAll("button")).find(
        (node) => node.textContent === "고급",
      );
      if (found === undefined) throw new Error("waiting for 고급");
      return found;
    });
    await new Promise((resolve) => setTimeout(resolve, 50));
    for (const phrase of editorPhrases) {
      assert.equal(view.container.textContent.includes(phrase), false, phrase);
    }
    assert.equal(trigger.getAttribute("aria-expanded"), "false");
    w.fireEvent.click(trigger);
    for (const phrase of editorPhrases) {
      await until(w, view, phrase);
    }
    assert.equal(
      Array.from(view.container.querySelectorAll("button")).filter(
        (node) => node.textContent === "고급",
      ).length,
      1,
    );
    w.cleanup();
  });
}
