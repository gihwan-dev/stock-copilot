import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

test("U-23 settings screen shows only 3 basic items by default, hides advanced behind Disclosure, shows optional key notice, and main window keeps macOS native titlebar", async () => {
  const tauriConf = JSON.parse(
    readFileSync(join(desktopRoot, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const mainWindow = tauriConf.app.windows[0];
  assert.equal(mainWindow.label, "main");
  assert.equal("decorations" in mainWindow, false);
  assert.equal("titleBarStyle" in mainWindow, false);

  const bundle = await screenDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, cleanup, waitFor } from "@testing-library/react";
    import { clearMocks } from "@tauri-apps/api/mocks";
    import { App } from APP_ENTRY;
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { responses } from RESPONSES_ENTRY;

    window.cleanup = cleanup;
    window.clearMocks = clearMocks;

    window.runSettingsStates = async () => {
      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings: () => new Promise(() => {}),
      });
      const loadingView = render(
        h(App, { initialPath: "/settings", queryClient: createAppQueryClient() }),
      );
      await waitFor(() => {
        if (!(loadingView.container.textContent ?? "").includes("설정을 불러오는 중이에요")) {
          throw new Error("loading state missing");
        }
      });
      const loadingText = loadingView.container.textContent ?? "";
      loadingView.unmount();

      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings: () => {
          throw responses.ipcErrors[1];
        },
      });
      const errorView = render(
        h(App, { initialPath: "/settings", queryClient: createAppQueryClient() }),
      );
      await waitFor(() => {
        if (!(errorView.container.textContent ?? "").includes("설정을 불러오지 못했어요")) {
          throw new Error("error state missing");
        }
      });
      const errorText = errorView.container.textContent ?? "";
      errorView.unmount();

      let orderToggleCalls = 0;
      mockCommands({
        appVersion: () => responses.appVersion,
        getSettings: () => responses.getSettings,
        setOrderPermission: () => {
          orderToggleCalls += 1;
          return responses.setOrderPermission;
        },
      });
      const readyView = render(
        h(App, { initialPath: "/settings", queryClient: createAppQueryClient() }),
      );
      await waitFor(() => {
        if (!(readyView.container.textContent ?? "").includes("토스 연결")) {
          throw new Error("ready state missing");
        }
      });
      const collapsedHeadings = Array.from(
        readyView.container.querySelectorAll("h3"),
        (el) => el.textContent,
      );
      const collapsedText = readyView.container.textContent ?? "";

      const orderSwitch = readyView.getByRole("switch", { name: "주문 켜기" });
      fireEvent.click(orderSwitch);
      await waitFor(() => {
        if (orderToggleCalls !== 1) throw new Error("order switch did not invoke IPC");
      });
      await waitFor(() => {
        if (!(readyView.container.textContent ?? "").includes("확인 창에서 취소되어")) {
          throw new Error("denied feedback missing");
        }
      });

      const advancedButton = readyView.getByRole("button", { name: "고급" });
      fireEvent.click(advancedButton);
      await waitFor(() => {
        if (!(readyView.container.textContent ?? "").includes("키를 넣으면 켜집니다")) {
          throw new Error("optional key text missing after expanding advanced");
        }
      });
      const expandedHeadings = Array.from(
        readyView.container.querySelectorAll("h3"),
        (el) => el.textContent,
      );
      const expandedText = readyView.container.textContent ?? "";

      fireEvent.click(readyView.getByRole("button", { name: "상태 보기" }));
      await waitFor(() => {
        if (!(readyView.container.textContent ?? "").includes("상태")) {
          throw new Error("did not navigate to status screen");
        }
      });
      const statusHeading = readyView.container.querySelector("h1")?.textContent ?? "";
      readyView.unmount();

      return {
        loadingText,
        errorText,
        collapsedHeadings,
        collapsedText,
        expandedHeadings,
        expandedText,
        orderToggleCalls,
        statusHeading,
      };
    };
  `);
  try {
    const win = bundle.open();
    const result = await win.runSettingsStates();
    assert.ok(result.loadingText.includes("설정을 불러오는 중이에요"));
    assert.ok(result.errorText.includes("설정을 불러오지 못했어요"));
    assert.deepEqual(Array.from(result.collapsedHeadings), [
      "토스 연결",
      "투자 성향",
      "주문 켜기",
    ]);
    assert.equal(result.collapsedText.includes("키를 넣으면 켜집니다"), false);
    assert.equal(result.orderToggleCalls, 1);
    assert.deepEqual(Array.from(result.expandedHeadings), [
      "토스 연결",
      "투자 성향",
      "주문 켜기",
      "AI",
      "뉴스·데이터",
      "투자 원칙",
      "시스템",
    ]);
    assert.ok(result.expandedText.includes("키를 넣으면 켜집니다"));
    assert.equal(result.expandedText.includes("제공자 선택"), false);
    assert.equal(result.statusHeading, "상태");
  } finally {
    bundle.close();
  }
});
