import assert from "node:assert/strict";
import test from "node:test";
import { fixtureSource, waitForText } from "./screen-fixtures.mjs";
import { screenDom } from "./screen-dom.mjs";

test("U-16 with the network offline the app still shows stored holdings and transactions from the local core", async () => {
  const bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, cleanup, waitFor } from "@testing-library/react";
    import { clearMocks } from "@tauri-apps/api/mocks";
    import { onlineManager } from "@tanstack/react-query";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    ${fixtureSource}
    ${waitForText}
    window.cleanup = cleanup;
    window.clearMocks = clearMocks;
    window.click = (element) => fireEvent.click(element);
    window.goOffline = () => {
      Object.defineProperty(window.navigator, "onLine", { value: false, configurable: true });
      window.dispatchEvent(new window.Event("offline"));
      onlineManager.setOnline(false);
      return onlineManager.isOnline();
    };
    window.mount = () => {
      mockCommands({ appVersion: () => responses.appVersion });
      const stored = {
        recordedAt: fx.at("2026-11-24T15:30:00+09:00"),
        connection: "stored",
        totalApproximate: fx.krw("5000000"),
        exchangeRate: undefined,
        currencies: [{ currency: "KRW", holdings: fx.krw("4800000"), buyingPower: fx.krw("200000") }],
        holdings: [
          {
            id: fx.id("h-1"),
            name: fx.free("저장된예시종목"),
            ticker: fx.ticker("000660"),
            quantity: fx.qty("10"),
            value: fx.krw("4800000"),
            profit: { amount: fx.krw("0"), rate: fx.pct("0"), direction: "flat" },
          },
        ],
        today: { amount: fx.krw("0"), rate: fx.pct("0"), direction: "flat" },
      };
      const transactions = {
        recordedAt: fx.at("2026-11-24T15:30:00+09:00"),
        rows: [
          {
            id: fx.id("t-1"),
            settledAt: fx.at("2026-11-24T00:00:00+09:00"),
            name: fx.free("저장된예시종목"),
            kind: "buy",
            quantity: fx.qty("10"),
            amount: fx.krw("4800000"),
            presence: "present",
          },
        ],
      };
      return render(
        h(App, {
          initialPath: "/",
          queryClient: createAppQueryClient(),
          sources: fx.sources({
            portfolio: () => fx.ok(stored),
            transactions: () => fx.ok(transactions),
          }),
        }),
      );
    };
  `);
  try {
    const w = bundle.open();
    assert.equal(w.goOffline(), false);
    const view = w.mount();
    const text = await w.waitText(view, "저장된예시종목");
    assert.ok(
      text.includes("인터넷에 연결되지 않아 저장된 자료를 보여 드리고 있어요."),
    );
    assert.ok(text.includes("4,800,000원"));
    w.click(view.getByRole("button", { name: "거래내역" }));
    const history = await w.waitText(view, "결제일 기준 거래내역");
    assert.ok(history.includes("매수"));
    w.cleanup();
  } finally {
    bundle.close();
  }
});
