import assert from "node:assert/strict";
import test from "node:test";
import { fixtureSource, waitForText } from "./screen-fixtures.mjs";
import { screenDom } from "./screen-dom.mjs";

test("U-11 transactions screen shows loading, empty, error and ready states and marks rows that vanished from Toss", async () => {
  const bundle = await screenDom(`
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
    window.click = (element) => fireEvent.click(element);
    const rows = [
      {
        id: fx.id("tx-1"),
        settledAt: fx.at("2026-11-20T00:00:00+09:00"),
        name: fx.free("예시전자"),
        kind: "buy",
        quantity: fx.qty("5"),
        amount: fx.krw("350000"),
        presence: "present",
      },
      {
        id: fx.id("tx-2"),
        settledAt: fx.at("2026-11-21T00:00:00+09:00"),
        name: fx.free("예시배당주"),
        kind: "dividend",
        quantity: undefined,
        amount: fx.krw("12000"),
        presence: "vanished",
      },
    ];
    window.mount = (transactions) => {
      mockCommands({ appVersion: () => responses.appVersion });
      return render(
        h(App, {
          initialPath: "/transactions",
          queryClient: createAppQueryClient(),
          sources: fx.sources({ transactions }),
        }),
      );
    };
    window.scenarios = {
      loading: fx.never,
      empty: () => fx.ok({ recordedAt: fx.at("2026-11-25T09:00:00+09:00"), rows: [] }),
      noData: () => fx.fail("no-data"),
      error: () => fx.fail("retryable"),
      ready: () => fx.ok({ recordedAt: fx.at("2026-11-25T09:00:00+09:00"), rows }),
    };
  `);
  try {
    const w = bundle.open();

    const loading = w.mount(w.scenarios.loading);
    await w.waitText(loading, "거래내역을 불러오는 중이에요");
    w.cleanup();

    for (const scenario of [w.scenarios.empty, w.scenarios.noData]) {
      const empty = w.mount(scenario);
      await w.waitText(empty, "아직 거래내역이 없어요");
      w.cleanup();
    }

    const failed = w.mount(w.scenarios.error);
    await w.waitText(failed, "거래내역을 불러오지 못했어요");
    assert.ok(failed.getByRole("button", { name: "다시 시도" }));
    w.cleanup();

    const ready = w.mount(w.scenarios.ready);
    const text = await w.waitText(ready, "예시배당주");
    const body = Array.from(
      ready.container.querySelectorAll("tbody tr"),
      (row) => row.textContent,
    );
    assert.equal(body.length, 2);
    assert.ok(body[0].includes("매수"));
    assert.ok(body[0].includes("정상"));
    assert.equal(body[0].includes("토스에서 사라짐"), false);
    assert.ok(body[1].includes("배당"));
    assert.ok(body[1].includes("토스에서 사라짐"));
    assert.ok(text.includes("지우지 않고"));
    w.click(ready.getByRole("button", { name: "현황으로" }));
    await w.waitText(ready, "내 현황");
    w.cleanup();
  } finally {
    bundle.close();
  }
});
