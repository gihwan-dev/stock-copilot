import assert from "node:assert/strict";
import test from "node:test";
import { fixtureSource, waitForText } from "./screen-fixtures.mjs";
import { screenDom } from "./screen-dom.mjs";

const notices = {
  aiContent:
    "이 분석과 제안은 AI가 만들었습니다. 사실과 다르거나 오래된 내용이 있을 수 있습니다. 숫자는 앱이 계산한 값이고, 근거 글은 AI가 쓴 것입니다. 투자 판단과 그 결과에 대한 책임은 본인에게 있습니다.",
  signalCard: "AI 제안 · 원금 손실 가능 · 승인 전 직접 확인",
  chatData:
    "AI를 쓰면 보유 주식·비중·손익·거래내역·투자 원칙·노트가 선택한 AI 제공자(Anthropic 또는 OpenAI)의 서버로 전송됩니다. 토스 키와 전체 계좌번호는 보내지 않습니다.",
  appInfo:
    "개인 본인용 도구입니다. 금융투자업자(투자자문·투자일임)가 아니며, 다른 사람에게 투자 조언을 제공하지 않습니다.",
};

test("U-17 fixed notices: shared catalog holds the four texts, app info shows on settings and the chat head shows the data transfer notice", async () => {
  const bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, cleanup, waitFor } from "@testing-library/react";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    import { text } from LIB_ENTRY;
    ${fixtureSource}
    ${waitForText}
    window.cleanup = cleanup;
    window.catalog = () => ({
      aiContent: text("notice.aiContent", "ko"),
      signalCard: text("notice.signalCard", "ko"),
      chatData: text("notice.chatData", "ko"),
      appInfo: text("notice.appInfo", "ko"),
    });
    window.mount = (initialPath, getSettings) => {
      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings,
      });
      return render(
        h(App, {
          initialPath,
          queryClient: createAppQueryClient(),
          sources: fx.sources({}),
        }),
      );
    };
    window.ready = () => responses.getSettings;
    window.failing = () => {
      throw responses.ipcErrors[1];
    };
  `);
  try {
    const w = bundle.open();
    assert.deepEqual({ ...w.catalog() }, notices);

    const settings = w.mount("/settings", w.ready);
    const settingsText = await w.waitText(settings, "토스 연결");
    assert.ok(settingsText.includes("앱 정보"));
    assert.ok(settingsText.includes(notices.appInfo));
    w.cleanup();

    const failed = w.mount("/settings", w.failing);
    const failedText = await w.waitText(failed, "설정을 불러오지 못했어요");
    assert.ok(failedText.includes(notices.appInfo));
    w.cleanup();

    const home = w.mount("/", w.ready);
    const homeText = await w.waitText(home, notices.chatData);
    assert.equal(homeText.includes(notices.appInfo), false);
    w.cleanup();
  } finally {
    bundle.close();
  }
});
