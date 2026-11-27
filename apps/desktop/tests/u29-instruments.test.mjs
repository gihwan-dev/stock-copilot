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
  window.type = (element, value) => fireEvent.change(element, { target: { value } });

  const rows = () => ({
    rows: [
      { id: fx.id("i-1"), name: fx.free("예시전자"), ticker: fx.ticker("005930"), origin: "holding", todayRate: fx.pct("1.25"), direction: "up" },
      { id: fx.id("i-2"), name: fx.free("예시바이오"), ticker: fx.ticker("207940"), origin: "watchlist", todayRate: fx.pct("-0.50"), direction: "down" },
    ],
  });
  const detail = (holding) => ({
    id: fx.id("i-1"),
    name: fx.free("예시전자"),
    ticker: fx.ticker("005930"),
    price: fx.krw("71200"),
    todayRate: fx.pct("1.25"),
    direction: "up",
    quotedAt: fx.at("2026-11-25T10:00:00+09:00"),
    candles: [
      { id: fx.id("c-1"), label: fx.free("11-24"), open: fx.dec("70000"), high: fx.dec("71500"), low: fx.dec("69800"), close: fx.dec("70300") },
      { id: fx.id("c-2"), label: fx.free("11-25"), open: fx.dec("70300"), high: fx.dec("71400"), low: fx.dec("70100"), close: fx.dec("71200") },
    ],
    holding: holding
      ? { quantity: fx.qty("12"), value: fx.krw("854400"), profit: fx.krw("54400"), profitRate: fx.pct("6.80") }
      : undefined,
  });
  const thesis = () => ({
    claims: [
      { id: fx.id("cl-1"), text: fx.free("메모리 가격 반등"), status: "confirmed" },
      { id: fx.id("cl-2"), text: fx.free("웹에서 본 수주 소식"), status: "needs-review" },
    ],
    history: [
      { id: fx.id("h-1"), at: fx.at("2026-11-24T20:00:00+09:00"), summary: fx.free("수주 주장 추가") },
    ],
  });

  window.calls = { added: [], removed: [] };
  window.mount = (overrides) => {
    window.calls = { added: [], removed: [] };
    mockCommands({ appVersion: () => responses.appVersion });
    return render(
      h(App, {
        initialPath: "/",
        queryClient: createAppQueryClient(),
        sources: fx.sources({
          addWatchlist: (...args) => {
            window.calls.added.push(args);
            return fx.ok({ kind: "rejected", reason: fx.free("관심 종목은 200개까지예요") });
          },
          removeWatchlist: (...args) => {
            window.calls.removed.push(args);
            return fx.ok({ kind: "applied" });
          },
          ...overrides,
        }),
      }),
    );
  };
  window.scenarios = {
    listLoading: () => ({ instruments: fx.never }),
    listEmpty: () => ({ instruments: () => fx.ok({ rows: [] }) }),
    listError: () => ({ instruments: () => fx.fail("retryable") }),
    ready: (holding, thesisLoad) => ({
      instruments: () => fx.ok(rows()),
      instrumentDetail: () => fx.ok(detail(holding)),
      thesis: thesisLoad ?? (() => fx.ok(thesis())),
    }),
    detailLoading: () => ({ instruments: () => fx.ok(rows()), instrumentDetail: fx.never }),
    detailEmpty: () => ({ instruments: () => fx.ok(rows()) }),
    detailError: () => ({ instruments: () => fx.ok(rows()), instrumentDetail: () => fx.fail("bug") }),
  };
`;

const settle = () => new Promise((resolve) => setTimeout(resolve, 30));

test("U-29 sidebar lists holdings and watchlist with today's move in four states and has no screen menu", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const loading = w.mount(w.scenarios.listLoading());
    const loadingText = await w.waitText(loading, "내 현황");
    assert.equal(loadingText.includes("표시할 종목이 없어요"), false);
    assert.equal(loadingText.includes("종목 목록을 불러오지 못했어요"), false);
    assert.equal(loadingText.includes("상세 보기"), false);
    w.cleanup();

    const empty = w.mount(w.scenarios.listEmpty());
    await w.waitText(empty, "표시할 종목이 없어요");
    w.cleanup();

    const failed = w.mount(w.scenarios.listError());
    await w.waitText(failed, "종목 목록을 불러오지 못했어요");
    w.cleanup();

    const ready = w.mount(w.scenarios.ready(true));
    const text = await w.waitText(ready, "예시바이오");
    assert.ok(text.includes("예시전자"));
    assert.ok(text.includes("보유"));
    assert.ok(text.includes("관심"));
    assert.ok(text.includes("1.25"));
    assert.equal(ready.queryAllByRole("menuitem").length, 0);
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("U-13 pressing an instrument opens quote, chart and holding in the body while the list and chat destination stay", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const view = w.mount(w.scenarios.ready(true));
    await w.waitText(view, "예시바이오");
    w.click(view.getAllByRole("button", { name: "상세 보기" })[0]);
    const text = await w.waitText(view, "일봉 차트");
    for (const label of [
      "시세",
      "현재가",
      "71,200",
      "오늘 등락",
      "내 보유",
      "854,400",

      "예시바이오",
    ]) {
      assert.ok(text.includes(label), label);
    }
    assert.ok(view.getByRole("button", { name: "채팅", exact: true }));
    w.click(view.getAllByRole("button", { name: "닫기" })[0]);
    await w.waitText(view, "내 현황");
    w.cleanup();

    const notHeld = w.mount(w.scenarios.ready(false));
    await w.waitText(notHeld, "예시바이오");
    w.click(notHeld.getAllByRole("button", { name: "상세 보기" })[0]);
    await w.waitText(notHeld, "이 종목은 보유하고 있지 않아요");
    w.cleanup();

    for (const [scenario, needle] of [
      ["detailLoading", "종목 정보를 불러오는 중이에요"],
      ["detailEmpty", "이 종목의 시세가 아직 없어요"],
      ["detailError", "종목 정보를 불러오지 못했어요"],
    ]) {
      const state = w.mount(w.scenarios[scenario]());
      await w.waitText(state, "예시바이오");
      w.click(state.getAllByRole("button", { name: "상세 보기" })[0]);
      await w.waitText(state, needle);
      w.cleanup();
    }
  } finally {
    bundle.close();
  }
});

test("J-08 thesis notes show the current claims and history in four states", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const open = async (thesisLoad) => {
      const view = w.mount(w.scenarios.ready(true, thesisLoad));
      await w.waitText(view, "예시바이오");
      w.click(view.getAllByRole("button", { name: "상세 보기" })[0]);
      return view;
    };
    const ready = await open(undefined);
    const text = await w.waitText(ready, "메모리 가격 반등");
    assert.ok(text.includes("현재 노트"));
    assert.ok(text.includes("확인 필요"));
    assert.ok(text.includes("바뀐 기록"));
    assert.ok(text.includes("수주 주장 추가"));
    w.cleanup();

    const loading = await open(w.fx.never);
    await w.waitText(loading, "논거 노트를 불러오는 중이에요");
    w.cleanup();

    const empty = await open(() => w.fx.ok({ claims: [], history: [] }));
    await w.waitText(empty, "아직 적힌 논거가 없어요");
    w.cleanup();

    const failed = await open(() => w.fx.fail("bug"));
    await w.waitText(failed, "논거 노트를 불러오지 못했어요");
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("J-09 watchlist add validates the code, sends only the ticker and shows the core's refusal; remove sends the instrument", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const view = w.mount(w.scenarios.ready(true));
    await w.waitText(view, "예시바이오");
    const field = view.getByLabelText("관심 종목 추가 (종목 코드)");
    w.type(field, "12");
    w.click(view.getByRole("button", { name: "추가" }));
    await w.waitText(view, "종목 코드 6자리를 넣어 주세요.");
    assert.equal(w.calls.added.length, 0);

    w.type(field, "000660");
    w.click(view.getByRole("button", { name: "추가" }));
    await w.waitText(view, "관심 종목은 200개까지예요");
    assert.equal(w.calls.added.length, 1);
    assert.equal(w.calls.added[0].length, 1);
    assert.equal(String(w.calls.added[0][0]), "000660");

    w.click(view.getAllByRole("button", { name: "상세 보기" })[0]);
    await w.waitText(view, "관심 종목에서 빼기");
    w.click(view.getByRole("button", { name: "관심 종목에서 빼기" }));
    await settle();
    assert.equal(w.calls.removed.length, 1);
    assert.equal(String(w.calls.removed[0][0]), "i-1");
    w.cleanup();
  } finally {
    bundle.close();
  }
});
