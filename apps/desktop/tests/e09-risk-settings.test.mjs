import assert from "node:assert/strict";
import test from "node:test";
import { fixtureSource, waitForText } from "./screen-fixtures.mjs";
import { screenDom } from "./screen-dom.mjs";
import { openAdvancedSource } from "./settings-advanced.mjs";

const bundleSource = `
  import { createElement as h } from "react";
  import { render, fireEvent, cleanup, waitFor } from "@testing-library/react";
  import { clearMocks } from "@tauri-apps/api/mocks";
  import { App } from APP_ENTRY;
  import { createAppQueryClient } from QUERY_ENTRY;
  import { mockCommands } from MOCK_IPC_ENTRY;
  import { responses } from RESPONSES_ENTRY;
  ${fixtureSource}
  ${openAdvancedSource}
  ${waitForText}
  window.cleanup = cleanup;
  window.clearMocks = clearMocks;
  window.click = (element) => fireEvent.click(element);
  window.pick = (element, value) => fireEvent.change(element, { target: { value } });

  window.saved = { profile: [], order: [] };
  window.mount = (overrides, outcome) => {
    window.saved = { profile: [], order: [] };
    mockCommands({ appVersion: () => responses.appVersion });
    window.openAdvanced();
    return render(
      h(App, {
        initialPath: "/settings",
        queryClient: createAppQueryClient(),
        sources: fx.sources({
          saveInvestorProfile: (...args) => {
            window.saved.profile.push(args);
            return fx.ok(outcome);
          },
          saveOrderSettings: (...args) => {
            window.saved.order.push(args);
            return fx.ok(outcome);
          },
          ...overrides,
        }),
      }),
    );
  };
  window.stored = {
    profile: () => fx.ok({
      horizon: "few-days",
      riskAppetite: "conservative",
      instrumentCap: "15",
      sectorCap: "30",
      excludedTypes: ["etf"],
    }),
    order: () => fx.ok({
      marketOrders: "on",
      hoursHorizon: "off",
      cardExpiry: "15",
      orderCountLimit: "5",
    }),
  };
`;

const open = async (w, view) => {
  await w.waitText(view, "투자 성향과 주문 설정 바꾸기");
  w.click(view.getByRole("button", { name: "투자 성향과 주문 설정 바꾸기" }));
};

const settle = () => new Promise((resolve) => setTimeout(resolve, 30));

test("E-09 and O-14 editors stay folded and show loading, error, defaults and stored values", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const folded = w.mount({}, { kind: "saved" });
    const foldedText = await w.waitText(folded, "투자 성향과 주문 설정 바꾸기");
    assert.equal(foldedText.includes("투자 성향 바꾸기"), false);
    w.cleanup();

    const loading = w.mount(
      { investorProfile: w.fx.never, orderSettings: w.fx.never },
      { kind: "saved" },
    );
    await open(w, loading);
    const loadingText = await w.waitText(loading, "투자 성향 바꾸기");
    assert.equal(loadingText.includes("저장"), false);
    w.cleanup();

    const failed = w.mount(
      {
        investorProfile: () => w.fx.fail("bug"),
        orderSettings: () => w.fx.fail("bug"),
      },
      { kind: "saved" },
    );
    await open(w, failed);
    await w.waitText(failed, "설정을 불러오지 못했어요");
    w.cleanup();

    const defaults = w.mount({}, { kind: "saved" });
    await open(w, defaults);
    const defaultText = await w.waitText(defaults, "고급 · 주문");
    assert.ok(
      defaultText.includes("아직 저장한 값이 없어 기본값을 보여 드려요."),
    );
    assert.equal(defaults.getByLabelText("주문 카드 만료 시간").value, "10");
    assert.equal(defaults.getByLabelText("하루 주문 횟수 상한").value, "off");
    assert.equal(
      defaults
        .getByRole("switch", { name: "시장가 허용" })
        .getAttribute("aria-checked"),
      "false",
    );
    assert.equal(
      defaults
        .getByRole("switch", { name: "몇 시간 단위 기간 허용" })
        .getAttribute("aria-checked"),
      "false",
    );
    assert.equal(defaults.getByLabelText("기간").value, "weeks-to-months");
    assert.equal(defaults.getByLabelText("위험 성향").value, "neutral");
    assert.equal(defaultText.includes("보정"), false);
    w.cleanup();

    const stored = w.mount(
      { investorProfile: w.stored.profile, orderSettings: w.stored.order },
      { kind: "saved" },
    );
    await open(w, stored);
    await w.waitText(stored, "고급 · 주문");
    assert.equal(stored.getByLabelText("기간").value, "few-days");
    assert.equal(stored.getByLabelText("주문 카드 만료 시간").value, "15");
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("E-09 saving sends the whole draft to the core and a declined confirmation keeps nothing", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    const view = w.mount(
      { investorProfile: w.stored.profile },
      { kind: "declined" },
    );
    await open(w, view);
    await w.waitText(view, "투자 성향 바꾸기");
    w.pick(view.getByLabelText("위험 성향"), "aggressive");
    w.pick(view.getByLabelText("종목 비중 상한"), "30");
    const saveButtons = view.getAllByRole("button", { name: "저장" });
    w.click(saveButtons[0]);
    await w.waitText(view, "확인 창에서 취소되어 이전 값을 그대로 둬요.");
    assert.equal(w.saved.profile.length, 1);
    const draft = w.saved.profile[0][0];
    assert.equal(draft.riskAppetite, "aggressive");
    assert.equal(draft.instrumentCap, "30");
    assert.equal(draft.horizon, "few-days");
    assert.equal(w.saved.order.length, 0);
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("O-14 saving order settings reports the core's outcome", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    for (const [outcome, needle] of [
      [{ kind: "saved" }, "저장했어요"],
      [
        { kind: "unavailable" },
        "지금은 확인 창을 열 수 없어 이전 값을 그대로 둬요.",
      ],
    ]) {
      const view = w.mount({}, outcome);
      await open(w, view);
      await w.waitText(view, "고급 · 주문");
      w.click(view.getByRole("switch", { name: "시장가 허용" }));
      const saveButtons = view.getAllByRole("button", { name: "저장" });
      w.click(saveButtons[1]);
      await w.waitText(view, needle);
      assert.equal(w.saved.order.length, 1);
      assert.equal(w.saved.order[0][0].marketOrders, "on");
      assert.equal(w.saved.order[0][0].cardExpiry, "10");
      w.cleanup();
    }
    await settle();
  } finally {
    bundle.close();
  }
});
