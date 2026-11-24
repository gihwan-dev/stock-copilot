import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";
import { desktopRoot } from "./paths.mjs";

test("DS08 AppShell renders stock list, main, and chat without screen-switch menu and handles overlay Escape", async () => {
  const bundle = await componentsDom(`
    import { createElement as h } from "react";
    import { render, fireEvent, cleanup } from "@testing-library/react";
    import { AppShell, Page, Section, Stack, Text, StatusBar } from DS_ENTRY;
    import { text } from LIB_ENTRY;
    window.cleanup = cleanup;
    window.fireKey = (element, key) => fireEvent.keyDown(element, { key });
    window.click = (element) => fireEvent.click(element);
    window.renderShell = (options = {}) => {
      const chatChanges = [];
      const listChanges = [];
      const view = render(
        h(
          AppShell,
          {
            defaultChatOpen: options.defaultChatOpen,
            defaultListOpen: options.defaultListOpen,
            onChatOpenChange: (next) => chatChanges.push(next),
            onListOpenChange: (next) => listChanges.push(next),
            status: options.withStatus
              ? h(StatusBar, { tone: "warning" }, text("ds.statusWarning", "ko"))
              : undefined,
            list: h(
              Stack,
              { gap: "space-2" },
              h(Text, null, text("catalog.companyTwo", "ko")),
              h(Text, null, text("catalog.companyThree", "ko")),
            ),
            chat: h(
              Stack,
              { gap: "space-2" },
              h(Text, null, text("catalog.description", "ko")),
            ),
          },
          h(
            Page,
            { title: text("app.title", "ko") },
            h(
              Section,
              { title: text("catalog.holdings", "ko") },
              h(Text, null, text("catalog.company", "ko")),
            ),
          ),
        ),
      );
      return { view, chatChanges, listChanges };
    };
  `);
  try {
    const w = bundle.open();
    const { view, chatChanges, listChanges } = w.renderShell({
      withStatus: true,
    });
    const navs = view.container.querySelectorAll("nav");
    assert.equal(navs.length, 0);
    assert.ok(view.container.querySelector("main"));
    assert.ok(view.container.querySelector('aside[aria-label="종목 목록"]'));
    assert.ok(view.container.querySelector('aside[aria-label="AI 채팅"]'));
    assert.ok(view.container.textContent.includes("자료가 오래됐어요"));
    assert.ok(view.container.textContent.includes("예시생활건강"));
    assert.ok(view.container.textContent.includes("주식 코파일럿"));

    const openListButton = view.container.querySelector(
      'button[aria-label="종목 목록 펼치기"]',
    );
    assert.ok(openListButton);
    w.click(openListButton);
    assert.deepEqual(Array.from(listChanges), ["open"]);

    const openChatButton = Array.from(
      view.container.querySelectorAll("button"),
    ).find((button) => button.textContent === "채팅 열기");
    assert.ok(openChatButton);
    w.click(openChatButton);
    assert.deepEqual(Array.from(chatChanges), ["open"]);

    w.fireKey(view.container.firstElementChild, "Escape");
    assert.deepEqual(Array.from(chatChanges), ["open", "closed"]);
    assert.deepEqual(Array.from(listChanges), ["open", "closed"]);
    w.cleanup();
  } finally {
    bundle.close();
  }
});

test("DS08 Tauri main window enforces 960x640 minimum size", () => {
  const conf = JSON.parse(
    readFileSync(join(desktopRoot, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const mainWindow = conf.app.windows.find((win) => win.label === "main");
  assert.ok(mainWindow);
  assert.equal(mainWindow.minWidth, 960);
  assert.equal(mainWindow.minHeight, 640);
});
