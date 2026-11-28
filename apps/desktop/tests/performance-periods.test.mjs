import assert from "node:assert/strict";
import test from "node:test";
import { join } from "node:path";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

const bundleSource = `
  import { createElement as h } from "react";
  import { render, waitFor, cleanup } from "@testing-library/react";
  import { QueryClientProvider } from "@tanstack/react-query";
  import { createAppQueryClient } from QUERY_ENTRY;
  import { mockCommands } from MOCK_IPC_ENTRY;
  import { PerformanceSummary } from ${JSON.stringify(join(desktopRoot, "src/features/portfolio/ui/PerformanceCard.tsx"))};

  window.show = async (status) => {
    const lines = [["1m", "-3.71"], ["3m", "-3.93"], ["6m", null], ["1y", "-61.92"]]
      .map(([span, rate]) => ({
        span,
        ...(status === undefined ? {} : { status: rate === null ? status : "ready" }),
        period_return: rate === null ? null : { rate, direction: "down" },
        max_drawdown: null,
        from: null,
        to: null,
      }));
    mockCommands({ getPerformance: () => ({ lines }) });
    const client = createAppQueryClient();
    const view = render(h(QueryClientProvider, { client }, h(PerformanceSummary)));
    await waitFor(() => {
      if (!view.container.textContent.includes("1년")) throw new Error(view.container.textContent);
    });
    const text = view.container.textContent;
    cleanup();
    client.clear();
    return text;
  };
`;

test("only an active calculation is pending when six months is the sole missing return", async () => {
  const bundle = await screenDom(bundleSource);
  try {
    const w = bundle.open();
    for (const [state, label] of [
      [undefined, "계산 불가"],
      ["waiting", "계산 대기"],
      ["calculating", "계산 중"],
      ["missing-price", "종가 부족"],
      ["missing-fx", "환율 부족"],
      ["missing-cash", "현금 자료 부족"],
      ["insufficient-data", "자료 부족"],
      ["load-failed", "조회 실패"],
      ["non-positive-capital", "계산 기준액 0 이하"],
      ["unavailable", "계산 불가"],
    ]) {
      const text = await w.show(state);
      assert.ok(text.includes(`6개월${label}`), text);
      for (const rate of ["3.71%", "3.93%", "61.92%"])
        assert.ok(text.includes(rate), text);
      assert.equal(text.includes("계산 중"), state === "calculating", text);
    }
  } finally {
    bundle.close();
  }
});
