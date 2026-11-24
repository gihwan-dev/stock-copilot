import assert from "node:assert/strict";
import test from "node:test";
import { fixtureSource, waitForText } from "./screen-fixtures.mjs";
import { screenDom } from "./screen-dom.mjs";

const internalNames = [
  "MCP",
  "세션",
  "프로필",
  "격리",
  "큐",
  "보정",
  "사전 점검",
  "토큰",
];

test("U-21 status band shows one action sentence only when the self check reports a problem and nothing when it is clear", async () => {
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
    window.mount = (selfCheck) => {
      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings: () => responses.getSettings,
      });
      return render(
        h(App, {
          initialPath: "/",
          queryClient: createAppQueryClient(),
          sources: fx.sources({ selfCheck }),
        }),
      );
    };
    window.problem = (problem) => () => fx.ok({ kind: "problem", problem });
    window.clear = () => fx.ok({ kind: "clear" });
    window.failed = () => fx.fail("bug");
    window.bands = (view) =>
      Array.from(view.container.querySelectorAll('[role="status"], [role="alert"]'))
        .map((element) => element.textContent ?? "")
        .filter((value) => value.length > 0);
  `);
  try {
    const w = bundle.open();

    for (const source of [w.clear, w.failed, w.fx.never]) {
      const view = w.mount(source);
      await w.waitText(view, "앱이 준비됐어요.");
      await new Promise((resolve) => setTimeout(resolve, 20));
      assert.equal(
        view.container.textContent.includes("주세요"),
        false,
        "no problem sentence while clear, failed or loading",
      );
      w.cleanup();
    }

    const cases = [
      ["toss-key-invalid", "토스 키가 맞지 않아요.", "설정 열기"],
      ["ip-not-allowed", "허용 IP 관리에 추가해 주세요.", "설정 열기"],
      ["toss-key-shared", "다른 프로그램을 끄면 다시 연결돼요.", "설정 열기"],
      ["claude-missing", "Claude Code를 설치하면 AI 기능이 켜져요.", undefined],
      ["claude-signed-out", "Claude Code에 로그인하면", undefined],
      ["order-risk-notice", "위험 안내를 먼저 확인해야", undefined],
    ];
    for (const [problem, sentence, action] of cases) {
      const view = w.mount(w.problem(problem));
      const text = await w.waitText(view, sentence);
      for (const name of internalNames) {
        assert.equal(text.includes(name), false, problem + " leaks " + name);
      }
      const buttons = Array.from(
        view.container.querySelectorAll("button"),
        (b) => b.textContent,
      );
      assert.equal(
        buttons.includes("설정 열기"),
        action !== undefined,
        problem,
      );
      if (action !== undefined) {
        w.click(view.getByRole("button", { name: action }));
        await w.waitText(view, "현황으로");
      }
      w.cleanup();
    }

    const single = w.mount(w.problem("toss-key-invalid"));
    const singleText = await w.waitText(single, "토스 키가 맞지 않아요.");
    assert.equal(singleText.split("토스 키가 맞지 않아요.").length, 2);
    assert.equal(singleText.includes("Claude Code"), false);
    w.cleanup();
  } finally {
    bundle.close();
  }
});
