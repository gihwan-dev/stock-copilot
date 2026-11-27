import assert from "node:assert/strict";
import test from "node:test";
import { screenDom } from "./screen-dom.mjs";

test("U04 App boots with mockCommands, renders ready and error states without rejecting queryFn, and navigates", async () => {
  const bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, waitFor, cleanup, within } from "@testing-library/react";
    import { clearMocks } from "@tauri-apps/api/mocks";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;
    import userEvent from "@testing-library/user-event";
    window.user = userEvent.setup();
    window.cleanup = cleanup;
    window.clearMocks = clearMocks;
    window.mockCommands = mockCommands;
    window.responses = responses;
    window.waitFor = waitFor;
    window.within = within;
    window.click = (el) => fireEvent.click(el);
    window.mountScreen = (initialPath = "/") => {
      const client = createAppQueryClient();
      const view = render(h(App, { initialPath, queryClient: client }));
      return { view, client };
    };
  `);
  try {
    const w = bundle.open();

    w.mockCommands({
      listChatSessions: () => [],
      getChatHistory: () => ({
        session_id: "main-chat",
        entries: [],
        active_turn: null,
      }),
      getPortfolio: () => {
        throw { kind: "no-data" };
      },
      appVersion: () => w.responses.appVersion,
    });
    const first = w.mountScreen("/");
    await w.waitFor(() => {
      if (!first.view.container.textContent.includes("앱이 준비됐어요."))
        throw new Error("waiting for ready state");
    });
    assert.ok(
      first.view.container.querySelector('aside[aria-label="종목 목록"]'),
    );
    const navigation = w.within(
      first.view.getByRole("navigation", { name: "화면 이동" }),
    );
    const destination = (name) =>
      navigation.getByRole("button", { name, exact: true });
    assert.equal(first.view.container.querySelector("textarea"), null);
    w.click(destination("브리핑"));
    await w.waitFor(() =>
      assert.ok(first.view.getByRole("heading", { name: "브리핑" })),
    );
    destination("채팅").focus();
    await w.user.keyboard("{Enter}");
    await w.waitFor(() =>
      assert.ok(first.view.getByText("무엇이든 물어보세요")),
    );
    assert.equal(
      first.view.container.querySelectorAll("main textarea").length,
      1,
    );
    assert.equal(destination("채팅").getAttribute("aria-pressed"), "true");
    w.click(destination("대시보드"));
    await w.waitFor(() =>
      assert.ok(first.view.getByRole("heading", { name: "내 현황" })),
    );
    assert.equal(first.view.container.querySelector("textarea"), null);
    const queryState = first.client.getQueryState(["app-version"]);
    assert.equal(queryState.status, "success");
    assert.equal(queryState.data.kind, "ok");

    const settingsBtn = Array.from(
      first.view.container.querySelectorAll("button"),
    ).find((button) => button.textContent === "설정");
    assert.ok(settingsBtn);
    w.click(settingsBtn);
    await w.waitFor(() => {
      if (!first.view.container.textContent.includes("현황으로"))
        throw new Error("waiting for settings route");
    });
    w.cleanup();
    w.clearMocks();

    w.mockCommands({
      getPortfolio: () => {
        throw { kind: "no-data" };
      },
      appVersion: () => {
        throw w.responses.ipcErrors[0];
      },
    });
    const failed = w.mountScreen("/");
    await w.waitFor(() => {
      if (
        !failed.view.container.textContent.includes("화면을 불러오지 못했어요")
      )
        throw new Error("waiting for error state");
    });
    const failedState = failed.client.getQueryState(["app-version"]);
    assert.equal(failedState.status, "success");
    assert.equal(failedState.data.kind, "err");
    assert.equal(failedState.data.error.kind, "command");
    w.cleanup();
    w.clearMocks();

    w.mockCommands({
      getPortfolio: () => {
        throw { kind: "no-data" };
      },
      appVersion: () => ({ version: "bad" }),
    });
    const malformed = w.mountScreen("/");
    await w.waitFor(() => {
      if (
        !malformed.view.container.textContent.includes("앱과 연결하지 못했어요")
      )
        throw new Error("waiting for invalid-response state");
    });
    const badState = malformed.client.getQueryState(["app-version"]);
    assert.equal(badState.status, "success");
    assert.equal(badState.data.kind, "err");
    assert.equal(badState.data.error.kind, "invalid-response");
    w.cleanup();
    w.clearMocks();
  } finally {
    bundle.close();
  }
});
