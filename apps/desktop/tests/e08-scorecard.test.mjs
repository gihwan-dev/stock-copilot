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

  const combination = (id, model, verdict, judged) => ({
    id: fx.id(id),
    model: fx.free(model),
    horizon: fx.free("며칠"),
    judged: fx.dec(judged),
    verdict,
    brier: verdict === "ready" ? fx.dec("0.214") : undefined,
    bins: [
      { id: fx.id(id + "-b1"), label: fx.free("30~40"), meanPrediction: fx.dec("35"), hitRate: fx.dec("33"), count: fx.dec("12") },
      { id: fx.id(id + "-b2"), label: fx.free("40~50"), meanPrediction: fx.dec("45"), hitRate: fx.dec("47"), count: fx.dec("21") },
    ],
    signalReturn: verdict === "ready" ? fx.pct("3.4") : undefined,
    benchmarkReturn: verdict === "ready" ? fx.pct("1.2") : undefined,
    intervalLow: verdict === "ready" ? fx.pct("-0.8") : undefined,
    intervalHigh: verdict === "ready" ? fx.pct("7.1") : undefined,
    comparisons: fx.dec("3"),
  });

  window.mount = (scorecard) => {
    mockCommands({ appVersion: () => responses.appVersion });
    return render(
      h(App, {
        initialPath: "/",
        queryClient: createAppQueryClient(),
        sources: fx.sources({ scorecard }),
      }),
    );
  };
  window.scenarios = {
    loading: () => fx.never,
    empty: () => () => fx.ok({ combinations: [] }),
    error: () => () => fx.fail("retryable"),
    ready: () => () =>
      fx.ok({
        combinations: [
          combination("c-1", "예시모델 A", "ready", "42"),
          combination("c-2", "예시모델 B", "pending", "12"),
        ],
      }),
  };
`;

const open = async (w, view) => {
  await w.waitText(view, "성적표");
  w.click(view.getByRole("button", { name: "성적표" }));
};

test("E-08 scorecard shows loading, empty, error and per-model, per-horizon results without internal terms", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();

    const loading = w.mount(w.scenarios.loading());
    await open(w, loading);
    const loadingText = await w.waitText(loading, "성적표");
    assert.equal(loadingText.includes("판정 건수"), false);
    w.cleanup();

    const empty = w.mount(w.scenarios.empty());
    await open(w, empty);
    await w.waitText(empty, "아직 판정된 추정이 없어요");
    w.cleanup();

    const failed = w.mount(w.scenarios.error());
    await open(w, failed);
    await w.waitText(failed, "성적표를 불러오지 못했어요");
    w.cleanup();

    const ready = w.mount(w.scenarios.ready());
    await open(w, ready);
    const text = await w.waitText(ready, "예시모델 B");
    for (const label of [
      "예시모델 A",
      "며칠",
      "판정 건수",
      "Brier 점수",
      "0.214",
      "신뢰도 도표",
      "확률 구간",
      "실제 적중률",
      "건수",
      "30~40",
      "신호를 따랐을 때의 수익률 (가상 청산)",
      "벤치마크 수익률",
      "신뢰구간",
      "비교한 모델·프롬프트 수",
    ]) {
      assert.ok(text.includes(label), label);
    }
    assert.equal(text.includes("보정"), false);
    const pendingCount = text.split("판정 보류").length - 1;
    assert.equal(pendingCount, 1);
    w.cleanup();
  } finally {
    bundle.close();
  }
});
