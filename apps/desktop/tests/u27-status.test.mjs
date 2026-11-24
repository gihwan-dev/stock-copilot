import assert from "node:assert/strict";
import test from "node:test";
import { fixtureSource, waitForText } from "./screen-fixtures.mjs";
import { screenDom } from "./screen-dom.mjs";

test("U-27 advanced status shows check records with reasons, no-action states, source success times and AI usage in four states", async () => {
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
    const status = (aiRuntime, aiLimit) => ({
      records: [
        {
          id: fx.id("r-1"),
          at: fx.at("2026-11-25T08:00:00+09:00"),
          check: fx.free("AI 실행 환경"),
          detail: fx.free("격리 설정 확인에 실패했어요"),
        },
      ],
      aiRuntime,
      aiLimit,
      sources: [
        { id: fx.id("src-1"), name: fx.free("연합뉴스"), lastSuccess: fx.at("2026-11-25T09:10:00+09:00") },
        { id: fx.id("src-2"), name: fx.free("SEC EDGAR"), lastSuccess: undefined },
      ],
      aiUsage: [
        { id: fx.id("u-1"), task: fx.free("확률 추정"), tokens: fx.dec("12345"), seconds: fx.dec("42") },
      ],
    });
    window.mount = (statusSource) => {
      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings: () => responses.getSettings,
      });
      return render(
        h(App, {
          initialPath: "/settings/status",
          queryClient: createAppQueryClient(),
          sources: fx.sources({ status: statusSource }),
        }),
      );
    };
    window.scenarios = {
      loading: fx.never,
      empty: () => fx.fail("no-data"),
      error: () => fx.fail("retryable"),
      unavailable: () =>
        fx.ok(status(
          { kind: "unavailable", reason: fx.free("Claude Code 버전이 바뀌어 다시 확인 중") },
          { kind: "waiting", resumesAt: fx.at("2026-11-25T13:00:00+09:00") },
        )),
      ready: () => fx.ok(status({ kind: "ready" }, { kind: "available" })),
    };
  `);
  try {
    const w = bundle.open();

    const loading = w.mount(w.scenarios.loading);
    await w.waitText(loading, "상태를 불러오는 중이에요");
    w.cleanup();

    const empty = w.mount(w.scenarios.empty);
    await w.waitText(empty, "앱이 첫 점검을 마치면");
    w.cleanup();

    const failed = w.mount(w.scenarios.error);
    await w.waitText(failed, "상태를 불러오지 못했어요");
    assert.ok(failed.getByRole("button", { name: "다시 시도" }));
    w.cleanup();

    const waiting = w.mount(w.scenarios.unavailable);
    const waitingText = await w.waitText(waiting, "SEC EDGAR");
    assert.ok(waitingText.includes("AI 기능만 꺼 두었어요"));
    assert.ok(waitingText.includes("Claude Code 버전이 바뀌어 다시 확인 중"));
    assert.ok(waitingText.includes("재개 시각"));
    assert.ok(waitingText.includes("2026-11-25 13:00"));
    assert.ok(waitingText.includes("연합뉴스"));
    assert.ok(waitingText.includes("2026-11-25 09:10"));
    assert.ok(waitingText.includes("아직 성공 기록 없음"));
    assert.ok(waitingText.includes("격리 설정 확인에 실패했어요"));
    assert.ok(waitingText.includes("확률 추정"));
    assert.ok(waitingText.includes("12345"));
    w.cleanup();

    const ready = w.mount(w.scenarios.ready);
    const readyText = await w.waitText(ready, "한도 안에서 쓰고 있어요.");
    assert.equal(readyText.includes("재개 시각"), false);
    assert.equal(readyText.includes("AI 기능만 꺼 두었어요"), false);
    w.click(ready.getByRole("button", { name: "설정으로" }));
    await w.waitText(ready, "기본 설정");
    w.cleanup();
  } finally {
    bundle.close();
  }
});
