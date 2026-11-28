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
  window.click = (element) => fireEvent.click(element);

  const portfolio = () => ({
    recordedAt: fx.at("2026-11-25T09:30:00+09:00"),
    connection: "live",
    totalApproximate: fx.krw("123456789"),
    exchangeRate: { kind: "mid-rate", at: fx.at("2026-11-25T09:00:00+09:00") },
    currencies: [
      { currency: "KRW", holdings: fx.krw("100000000"), buyingPower: fx.krw("3456789") },
      { currency: "USD", holdings: fx.usd("14000.50"), buyingPower: fx.usd("120.25") },
    ],
    holdings: [
      {
        id: fx.id("holding-1"),
        name: fx.free("예시전자"),
        ticker: fx.ticker("005930"),
        quantity: fx.qty("12"),
        value: fx.krw("850000"),
        profit: { amount: fx.krw("50000"), rate: fx.pct("6.25"), direction: "up" },
      },
      {
        id: fx.id("holding-2"),
        name: fx.free("예시바이오"),
        ticker: fx.ticker("207940"),
        quantity: fx.qty("3"),
        value: fx.krw("2400000"),
        profit: { amount: fx.krw("-120000"), rate: fx.pct("-4.76"), direction: "down" },
      },
    ],
    today: { amount: fx.krw("-15000"), rate: fx.pct("-0.12"), direction: "down" },
  });

  const allocation = (concentration) => ({
    instruments: [
      { id: fx.id("a-1"), label: fx.free("예시바이오"), value: fx.krw("2400000"), weight: fx.pct("73.8"), level: "over-cap" },
      { id: fx.id("a-2"), label: fx.free("예시전자"), value: fx.krw("850000"), weight: fx.pct("26.2"), level: "normal" },
    ],
    sectors: [
      { id: fx.id("s-1"), label: fx.free("헬스케어"), value: fx.krw("2400000"), weight: fx.pct("73.8"), level: "warning" },
    ],
    concentration,
  });

  window.mount = (sources) => {
    mockCommands({ appVersion: () => responses.appVersion });
    return render(
      h(App, { initialPath: "/", queryClient: createAppQueryClient(), sources }),
    );
  };

  window.awaitSector = (view) => waitFor(() => view.getByText("반도체"), { timeout: 5000 });
  window.scenarios = {
    sectorRefresh: () => {
      let reads = 0;
      const fund = { id: fx.id("fund"), kind: "etf-fund", label: fx.free(""), value: fx.krw("50000"), weight: fx.pct("25"), level: null };
      const cash = { id: fx.id("cash"), kind: "cash", label: fx.free(""), value: fx.krw("0.1"), weight: fx.pct("0.01"), weightRange: "below-unit", level: null };
      const unknown = { id: fx.id("unknown"), kind: "unclassified", label: fx.free(""), value: fx.krw("150000"), weight: fx.pct("75"), level: null };
      return fx.sources({portfolio: () => fx.ok(portfolio()), allocation: () => {
        const waiting = reads++ === 0;
        const sector = {...unknown, kind: "sector", label: fx.free("반도체"), level: "over-cap"};
        return fx.ok({...allocation("balanced"), sectorRefresh: waiting ? "pending" : "settled", sectors: [fund, cash, waiting ? unknown : sector]});
      }});
    },
    converted: () => fx.sources({portfolio: () => fx.ok({
      ...portfolio(),
      totalApproximate:fx.krw("122458659"),
      usdConversion:{
        krwPerUsd:fx.krw("1345.67"), holdingsKrw:fx.krw("18840053"), buyingPowerKrw:fx.krw("161817")
      },
    })}),
    loading: () => fx.sources({ portfolio: fx.never }),
    empty: () => fx.sources({}),
    error: () => fx.sources({ portfolio: () => fx.fail("retryable") }),
    ready: (concentration) =>
      fx.sources({
        portfolio: () => fx.ok(portfolio()),
        allocation: () => fx.ok(allocation(concentration)),
      }),
    allocationError: () =>
      fx.sources({
        portfolio: () => fx.ok(portfolio()),
        allocation: () => fx.fail("bug"),
      }),
    allocationEmpty: () =>
      fx.sources({ portfolio: () => fx.ok(portfolio()) }),
    stored: () =>
      fx.sources({
        portfolio: () => fx.ok({ ...portfolio(), connection: "stored" }),
      }),
  };
`;

test("U-09 home overview shows loading, empty, error and ready states with approximate total, currencies, holdings and today's profit", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();

    const loading = w.mount(w.scenarios.loading());
    await w.waitText(loading, "현황을 불러오는 중이에요");
    w.cleanup();

    const empty = w.mount(w.scenarios.empty());
    const emptyText = await w.waitText(empty, "앱이 준비됐어요.");
    assert.ok(emptyText.includes("토스증권 계좌 자료를 받으면"));
    assert.equal(emptyText.includes("보유 종목"), false);
    w.cleanup();

    const failed = w.mount(w.scenarios.error());
    const failedText = await w.waitText(failed, "현황을 불러오지 못했어요");
    assert.ok(failed.getByRole("button", { name: "다시 시도" }));
    assert.equal(failedText.includes("총평가"), false);
    w.cleanup();

    const ready = w.mount(w.scenarios.ready("balanced"));
    const readyText = await w.waitText(ready, "예시바이오");
    assert.ok(readyText.includes("총평가"));
    assert.ok(readyText.includes("근사값"));
    assert.ok(readyText.includes("매매기준율 환산 시각"));
    assert.ok(readyText.includes("통화별 평가"));
    assert.ok(readyText.includes("원화"));
    assert.ok(readyText.includes("달러"));
    assert.ok(readyText.includes("오늘 손익"));
    assert.ok(readyText.includes("005930"));
    assert.ok(readyText.includes("850,000원"));
    assert.equal(readyText.includes("저장된 자료를 보여 드리고"), false);
    const rows = ready.container.querySelectorAll("tbody tr");
    assert.ok(rows.length >= 4);

    w.click(ready.getByRole("button", { name: "거래내역" }));
    await w.waitText(ready, "현황으로");
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("통화별 명세는 적용된 달러 환율과 두 원화 환산액을 그대로 보여준다", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const old = w.mount(w.scenarios.ready("balanced"));
    await w.waitText(old, "총평가");
    assert.equal(old.queryByText("1달러당"), null);
    assert.equal(old.queryByText("달러의 원화 환산액"), null);
    w.cleanup();
    const view = w.mount(w.scenarios.converted());
    await w.waitText(view, "달러의 원화 환산액");
    assert.ok(view.getByText("122,458,659원"));
    assert.ok(view.getByText("1,345.67원"));
    const row = view
      .getByText("달러의 원화 환산액", { exact: true })
      .closest("tr");
    assert.ok(row);
    assert.deepEqual(
      [...row.cells].map((cell) => cell.textContent),
      ["달러의 원화 환산액", "18,840,053원", "161,817원"],
    );
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("U-10 allocation and concentration show loading, empty, error and ready states with weights and cap levels", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();

    const balanced = w.mount(w.scenarios.ready("balanced"));
    const balancedText = await w.waitText(balanced, "종목별 비중");
    assert.ok(balancedText.includes("자산배분과 쏠림"));
    assert.ok(balancedText.includes("한쪽으로 쏠리지 않았어요."));
    assert.ok(balancedText.includes("섹터별 비중"));
    assert.ok(balancedText.includes("헬스케어"));
    assert.ok(balancedText.includes("초과"));
    assert.ok(balancedText.includes("주의"));
    assert.ok(balancedText.includes("여유"));
    w.cleanup();

    const concentrated = w.mount(w.scenarios.ready("concentrated"));
    const concentratedText = await w.waitText(
      concentrated,
      "몇 종목에 비중이 몰려 있어요",
    );
    assert.equal(concentratedText.includes("한쪽으로 쏠리지 않았어요."), false);
    w.cleanup();

    const updating = w.mount(w.scenarios.sectorRefresh());
    const initial = await w.waitText(updating, "업종을 확인하고 있어요");
    for (const label of [
      "예시바이오",
      "ETF·펀드",
      "미분류",
      "현금 (원화·달러)",
      "0.01% 미만",
      "해당 없음",
    ]) {
      assert.ok(initial.includes(label), label);
    }
    assert.equal(
      initial.includes("아직 자산배분을 계산할 자료가 없어요"),
      false,
    );
    await w.awaitSector(updating);
    assert.ok(
      updating.container.textContent.includes("원화로 합산한 총액 기준"),
    );
    assert.equal(
      updating.container.textContent.includes("업종을 확인하고 있어요"),
      false,
    );
    w.cleanup();

    const empty = w.mount(w.scenarios.allocationEmpty());
    await w.waitText(empty, "아직 자산배분을 계산할 자료가 없어요");
    w.cleanup();

    const failed = w.mount(w.scenarios.allocationError());
    const failedText = await w.waitText(failed, "자산배분을 불러오지 못했어요");
    assert.ok(failedText.includes("총평가"));
    w.cleanup();
  } finally {
    bundle.close();
  }
});
