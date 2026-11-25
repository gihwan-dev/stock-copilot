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

  const card = (overrides) => ({
    id: fx.id("card-1"),
    name: fx.free("예시전자"),
    ticker: fx.ticker("005930"),
    direction: "buy",
    market: "open",
    orderSwitch: "on",
    proposal: fx.id("proposal-1"),
    price: fx.krw("71200"),
    sizing: {
      kind: "sized",
      quantity: fx.qty("3"),
      amount: fx.krw("213600"),
      costs: fx.krw("320"),
      weightAfter: fx.pct("4.2"),
    },
    aiProbability: fx.pct("46"),
    trackedProbability: fx.pct("41"),
    threshold: fx.pct("38"),
    binHitRate: fx.pct("44"),
    binCount: fx.dec("12"),
    rationale: fx.free("실적 개선 근거"),
    counterargument: fx.free("환율 부담 반론"),
    sources: [
      { id: fx.id("s-1"), kind: "tool", label: fx.free("tool:q1") },
      { id: fx.id("s-2"), kind: "news", label: fx.free("예시 뉴스 기사") },
    ],
    ...overrides,
  });

  window.calls = { approval: [], skipped: [] };
  window.mount = (cards, extra) => {
    window.calls = { approval: [], skipped: [] };
    mockCommands({ appVersion: () => responses.appVersion });
    return render(
      h(App, {
        initialPath: "/",
        queryClient: createAppQueryClient(),
        sources: fx.sources({
          signals: cards === "never" ? fx.never : cards === "fail" ? () => fx.fail("retryable") : cards === "none" ? () => fx.ok({ cards: [] }) : () => fx.ok({ cards }),
          requestOrderApproval: (...args) => {
            window.calls.approval.push(args);
            return fx.ok(window.approvalAnswer ?? "requested");
          },
          skipSignal: (...args) => {
            window.calls.skipped.push(args);
            return fx.ok("recorded");
          },
          ...extra,
        }),
      }),
    );
  };
  window.card = card;
`;

const orderButtons = (view) =>
  view.queryAllByRole("button", { name: "주문" }).length;

test("E-06 signal cards show loading, empty, error and ready states with code facts apart from AI reasons", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();

    const loading = w.mount("never");
    await w.waitText(loading, "신호를 불러오는 중이에요");
    w.cleanup();

    const empty = w.mount("none");
    await w.waitText(empty, "지금은 신호가 없어요");
    w.cleanup();

    const failed = w.mount("fail");
    await w.waitText(failed, "신호를 불러오지 못했어요");
    w.cleanup();

    const ready = w.mount([w.card({})]);
    const collapsed = await w.waitText(ready, "매수 신호");
    assert.equal(collapsed.includes("판정 건수"), false);
    w.click(ready.getByRole("button", { name: "앱이 계산한 값" }));
    const text = await w.waitText(ready, "판정 건수");
    for (const label of [
      "앱이 계산한 값",
      "현재가",
      "수량",
      "금액",
      "예상 수수료·세금",
      "주문 후 비중",
      "AI가 낸 확률",
      "실제 적중률을 반영한 확률",
      "기준 확률",
      "이 확률 구간의 과거 적중률",
      "판정 건수",
      "AI가 낸 근거",
      "실적 개선 근거",
      "환율 부담 반론",
      "예시 뉴스 기사",
    ]) {
      assert.ok(text.includes(label), label);
    }
    assert.equal(text.includes("보정"), false);
    assert.equal(text.includes("tool:q1"), false);
    assert.equal(text.includes("외부 출처뿐"), false);
    assert.equal(orderButtons(ready), 1);
    w.click(ready.getByRole("button", { name: "주문" }));
    await new Promise((resolve) => setTimeout(resolve, 20));
    assert.equal(w.calls.approval.length, 1);
    assert.equal(w.calls.approval[0].length, 1);
    assert.equal(String(w.calls.approval[0][0]), "proposal-1");
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("E-06 order buttons appear only when the switch is on, the market is open, a proposal exists and the quantity is not zero", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    for (const overrides of [
      { orderSwitch: "off" },
      { proposal: undefined },
      { market: "closed" },
      {
        sizing: {
          kind: "zero",
          reason: w.fx.free("토스증권 앱에서 환전하면 주문할 수 있어요"),
        },
      },
    ]) {
      const view = w.mount([w.card(overrides)]);
      await w.waitText(view, "예시전자");
      assert.equal(
        orderButtons(view),
        0,
        JSON.stringify(Object.keys(overrides)),
      );
      assert.equal(view.queryAllByRole("spinbutton").length, 0);
      w.cleanup();
    }
    const zero = w.mount([
      w.card({
        sizing: {
          kind: "zero",
          reason: w.fx.free("토스증권 앱에서 환전하면 주문할 수 있어요"),
        },
      }),
    ]);
    const zeroText = await w.waitText(
      zero,
      "토스증권 앱에서 환전하면 주문할 수 있어요",
    );
    assert.equal(zeroText.includes("주문 후 비중"), false);
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("fin-gaps: a stored proposal with sizing still pending offers the order button and orders off explain how to turn them on", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const pending = w.mount([w.card({ sizing: { kind: "pending" } })]);
    await w.waitText(pending, "주문할 때 최신 호가로 계산");
    assert.equal(orderButtons(pending), 1, "제안이 있으면 주문 버튼");
    w.click(pending.getByRole("button", { name: "주문" }));
    await pending.findByText("예시전자");
    assert.equal(w.calls.approval.length, 1, "승인 요청은 제안 ID 하나만");
    assert.equal(w.calls.approval[0][0], "proposal-1");
    w.cleanup();
    w.approvalAnswer = "hourly-limit";
    const limited = w.mount([w.card({})]);
    await w.waitText(limited, "예시전자");
    w.click(limited.getByRole("button", { name: "주문" }));
    await w.waitText(limited, "한 시간에 낼 수 있는 주문 수를 넘었어요");
    w.cleanup();
    w.approvalAnswer = "presence-denied";
    const denied = w.mount([w.card({})]);
    await w.waitText(denied, "예시전자");
    w.click(denied.getByRole("button", { name: "주문" }));
    const deniedText = await w.waitText(denied, "본인 확인을 하지 않아");
    assert.equal(
      /presence|denied|[A-Z_]{4,}/u.test(
        deniedText.replace(/예시전자|005930|tool:q1/gu, ""),
      ),
      false,
      "내부 코드 노출 없음",
    );
    w.cleanup();
    w.approvalAnswer = undefined;
    const off = w.mount([w.card({ orderSwitch: "off" })]);
    const offText = await w.waitText(off, "주문 켜기");
    assert.equal(orderButtons(off), 0);
    assert.ok(offText.includes("설정"), "켜는 곳 안내");
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("E-07 a closed market shows the check-when-open card without buttons", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const view = w.mount([w.card({ market: "closed" })]);
    const text = await w.waitText(view, "장 열리면 확인");
    assert.ok(text.includes("장이 열리면"));
    assert.equal(view.queryAllByRole("button", { name: "주문" }).length, 0);
    assert.equal(view.queryAllByRole("button", { name: "넘기기" }).length, 0);
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("E-06 sources: tool references never count, none means no source, external-only adds a warning", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const toolOnly = w.mount([
      w.card({
        sources: [
          { id: w.fx.id("t"), kind: "tool", label: w.fx.free("tool:q1") },
        ],
      }),
    ]);
    const toolText = await w.waitText(toolOnly, "출처 없음");
    assert.equal(toolText.includes("외부 출처뿐"), false);
    w.cleanup();

    const external = w.mount([
      w.card({
        sources: [
          { id: w.fx.id("w"), kind: "web", label: w.fx.free("예시 웹 글") },
          {
            id: w.fx.id("d"),
            kind: "disclosure",
            label: w.fx.free("예시 공시"),
          },
        ],
      }),
    ]);
    const externalText = await w.waitText(external, "예시 웹 글");
    assert.ok(externalText.includes("외부 출처뿐"));
    w.cleanup();

    const none = w.mount([w.card({ sources: [] })]);
    await w.waitText(none, "출처 없음");
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("E-10 skip records a one-line reason through the skip action only", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const view = w.mount([w.card({ orderSwitch: "off" })]);
    await w.waitText(view, "넘기기");
    w.type(view.getByLabelText("넘기는 이유 (선택)"), "지금은 비싸다");
    w.click(view.getByRole("button", { name: "넘기기" }));
    await new Promise((resolve) => setTimeout(resolve, 20));
    assert.equal(w.calls.skipped.length, 1);
    assert.equal(String(w.calls.skipped[0][0]), "card-1");
    assert.equal(String(w.calls.skipped[0][1]), "지금은 비싸다");
    assert.equal(w.calls.approval.length, 0);
    w.cleanup();
  } finally {
    bundle.close();
  }
});
