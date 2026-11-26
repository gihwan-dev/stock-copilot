import assert from "node:assert/strict";
import test from "node:test";
import { fixtureSource, waitForText } from "./screen-fixtures.mjs";
import { screenDom } from "./screen-dom.mjs";

const bundleSource = `
  import { createElement as h } from "react";
  import { render, fireEvent, cleanup, waitFor } from "@testing-library/react";
  import { clearMocks } from "@tauri-apps/api/mocks";
  import { App } from APP_ENTRY;
  import { createAppQueryClient } from QUERY_ENTRY;
  import { mockCommands } from MOCK_IPC_ENTRY;
  import { responses } from RESPONSES_ENTRY;
  ${fixtureSource}
  ${waitForText}
  window.cleanup = cleanup;
  window.clearMocks = clearMocks;

  window.mount = (sources) => {
    mockCommands({ appVersion: () => responses.appVersion });
    return render(
      h(App, { initialPath: "/", queryClient: createAppQueryClient(), sources }),
    );
  };

  window.failWith = (kind, reason) =>
    fx.sources({
      portfolio: () =>
        Promise.resolve({
          kind: "err",
          error: { kind: "command", error: reason === undefined ? { kind } : { kind, reason } },
        }),
    });
`;

const logPath =
  "~/Library/Application Support/dev.stockcopilot.desktop/diagnostics.log";

test("home error explains the coarse sync failure reason and points to the diagnostics log", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const cases = [
      ["retryable", "network", "토스증권 서버에 연결하지 못했어요."],
      ["retryable", "server-busy", "토스증권 서버가 잠시 응답하지 않아요."],
      ["retryable", "response-mismatch", "앱 업데이트가 필요해요."],
      ["retryable", "rejected", "토스증권이 요청을 거절했어요."],
      ["retryable", "instruments-not-ready", "종목 정보를 아직 받지 못했어요."],
      ["retryable", "unknown", "토스증권 자료를 받는 중에 문제가 생겼어요."],
      ["retryable", null, "토스증권 자료를 받는 중에 문제가 생겼어요."],
      ["retryable", undefined, "토스증권 자료를 받는 중에 문제가 생겼어요."],
    ];
    for (const [kind, reason, sentence] of cases) {
      const view = w.mount(w.failWith(kind, reason));
      const shown = await w.waitText(view, "현황을 불러오지 못했어요");
      const body = await w.waitText(view, sentence);
      assert.ok(body.includes(logPath), `${reason}: ${body}`);
      assert.ok(view.getByRole("button", { name: "다시 시도" }));
      assert.equal(shown.includes("총평가"), false);
      w.cleanup();
    }
    const blocked = w.mount(w.failWith("user-fixable", undefined));
    const blockedText = await w.waitText(
      blocked,
      "토스증권 연결을 확인하지 못했어요.",
    );
    assert.equal(blockedText.includes(logPath), false);
    w.cleanup();
  } finally {
    bundle.close();
  }
});
