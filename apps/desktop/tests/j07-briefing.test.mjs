import assert from "node:assert/strict";
import test from "node:test";
import { fixtureSource, waitForText } from "./screen-fixtures.mjs";
import { screenDom } from "./screen-dom.mjs";

test("J-07 briefing card and past briefing screen show loading, empty, error and ready states from the core briefing IPC", async () => {
  const bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, cleanup, waitFor } from "@testing-library/react";
    import { clearMocks } from "@tauri-apps/api/mocks";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { getBriefings } from IPC_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    ${fixtureSource}
    ${waitForText}
    window.cleanup = cleanup;
    window.clearMocks = clearMocks;
    window.click = (element) => fireEvent.click(element);
    window.mount = (briefings, wire, initialPath) => {
      mockCommands({
        appVersion: () => responses.appVersion,
        getBriefings: () => wire ?? responses.getBriefings,
      });
      return render(
        h(App, {
          initialPath: initialPath ?? "/",
          queryClient: createAppQueryClient(),
          sources: fx.sources({ briefings }),
        }),
      );
    };
    window.ipc = getBriefings;
    window.scenarios = {
      loading: fx.never,
      empty: () => fx.ok({ latest: undefined, past: [] }),
      noData: () => fx.fail("no-data"),
      error: () => fx.fail("retryable"),
    };
    window.recorded = responses.getBriefings;
  `);
  try {
    const w = bundle.open();

    const loading = w.mount(w.scenarios.loading);
    await w.waitText(loading, "브리핑을 불러오는 중이에요");
    w.cleanup();

    for (const scenario of [w.scenarios.empty, w.scenarios.noData]) {
      const empty = w.mount(scenario);
      await w.waitText(empty, "아직 브리핑이 없어요");
      w.cleanup();
    }

    const failed = w.mount(w.scenarios.error);
    await w.waitText(failed, "브리핑을 불러오지 못했어요");
    assert.ok(failed.getByRole("button", { name: "다시 시도" }));
    w.cleanup();

    const invalid = w.mount(w.ipc, {
      ...w.recorded,
      latest: { ...w.recorded.latest, status: "completed", failure: "secret" },
    });
    await w.waitText(invalid, "브리핑을 불러오지 못했어요");
    w.cleanup();

    const ready = w.mount(w.ipc);
    const card = await w.waitText(ready, "밤사이 미국장은 올랐습니다");
    assert.ok(card.includes("아침 브리핑"));
    assert.ok(card.includes("연합 기사"));
    assert.ok(card.includes("뉴스"));
    assert.ok(card.includes("웹 요약"));
    assert.ok(card.includes("미검증 웹 값"));
    assert.ok(card.includes("매매 권유가 아니에요"));
    assert.equal(card.includes("마감 리뷰"), false);
    w.click(ready.getByRole("button", { name: "지난 브리핑 보기" }));
    const history = await w.waitText(ready, "이번 브리핑은 만들지 못했어요");
    assert.ok(history.includes("마감 리뷰"));
    assert.ok(history.includes("AI를 쓸 수 없어"));
    assert.equal(history.includes("밤사이 미국장은 올랐습니다"), false);
    w.click(ready.getByRole("button", { name: "현황으로" }));
    await w.waitText(ready, "밤사이 미국장은 올랐습니다");
    w.cleanup();

    const noPast = w.mount(w.ipc, { latest: w.recorded.latest, past: [] }, "/");
    await w.waitText(noPast, "밤사이 미국장은 올랐습니다");
    assert.equal(
      noPast.queryByRole("button", { name: "지난 브리핑 보기" }),
      null,
    );
    w.cleanup();

    const historyEmpty = w.mount(
      w.ipc,
      { latest: w.recorded.latest, past: [] },
      "/briefings",
    );
    await w.waitText(historyEmpty, "지난 브리핑이 없어요");
    w.cleanup();

    const historyFailed = w.mount(w.scenarios.error, undefined, "/briefings");
    await w.waitText(historyFailed, "브리핑을 불러오지 못했어요");
    w.cleanup();
  } finally {
    bundle.close();
  }
});
