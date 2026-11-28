import assert from "node:assert/strict";
import { join } from "node:path";
import test from "node:test";
import { chromium } from "@playwright/test";
import { build } from "vite";
import { desktopRoot } from "./paths.mjs";

const entry = join(desktopRoot, "chat-stop-fixture.tsx");
const fixture = `
import React from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { createAppQueryClient } from "./src/app/query";
import { ChatPanel } from "./src/features/chat";
import { mockCommands } from "./src/shared/ipc/testing/mockBindings";
import "./src/shared/ds/theme.css";
let finish;
window.sent = [];
window.cancelled = [];
mockCommands({
  getChatHistory: () => ({session_id:"main-chat", entries:[], active_turn:null}),
  sendChatMessage: ({request}) => {
    window.sent.push(request.turn_id);
    return new Promise(resolve => {
      finish = () => resolve({...request,status:"Cancelled",mode:"FreshAnalysis",
        actual_model:null,first_token_latency_ms:null,resets_at_unix_seconds:null,failure_reason:null});
    });
  },
  cancelChatTurn: ({request}) => {
    window.cancelled.push(request.turn_id);
    finish();
    return {turn_id:request.turn_id,outcome:"Signaled"};
  }
});
createRoot(document.querySelector("main")).render(
  <QueryClientProvider client={createAppQueryClient()}>
    <ChatPanel>{({messages,composer}) => <>
      <section style={{flex:"1",overflow:"auto"}}>{messages}</section>{composer}
    </>}</ChatPanel>
  </QueryClientProvider>
);`;

test("compact chat keeps its bounds through IME, send and cancellation", async () => {
  const bundle = await build({
    root: desktopRoot,
    logLevel: "silent",
    plugins: [
      {
        name: "chat-stop-fixture",
        resolveId: (id) => (id === entry ? entry : undefined),
        load: (id) => (id === entry ? fixture : undefined),
      },
    ],
    define: { "process.env.NODE_ENV": '"production"' },
    build: {
      write: false,
      lib: { entry, name: "ChatStopFixture", formats: ["iife"] },
    },
  });
  const output = [bundle].flat()[0].output;
  const browser = await chromium.launch();
  try {
    for (const width of [240, 320, 640]) {
      const page = await browser.newPage({ viewport: { width, height: 600 } });
      await page.route("http://chat.test/", (route) =>
        route.fulfill({
          contentType: "text/html",
          body: '<main style="height:100vh;display:flex;flex-direction:column"></main>',
        }),
      );
      await page.goto("http://chat.test/");
      await page.addStyleTag({
        content: output.find((item) => item.fileName.endsWith(".css")).source,
      });
      await page.addScriptTag({
        content: output.find((item) => item.type === "chunk").code,
      });
      const input = page.getByRole("textbox");
      await input.fill("질문");
      const bounds = async () =>
        Promise.all([
          input.boundingBox(),
          page.locator("main > section").boundingBox(),
        ]);
      const ready = await bounds();
      await input.dispatchEvent("compositionstart");
      await input.press("Enter");
      await input.dispatchEvent("compositionend");
      await input.press("Enter");
      await input.press("Shift+Enter");
      assert.match(await input.inputValue(), /\n/);
      assert.deepEqual(await page.evaluate(() => window.sent), []);
      await input.press("Enter");
      const stop = page.getByRole("button", { name: "멈추기" });
      await stop.waitFor();
      assert.deepEqual(await bounds(), ready);
      const left = await stop.boundingBox();
      const right = await page
        .getByRole("button", { name: "보내는 중" })
        .boundingBox();
      assert.equal(left.y, right.y);
      assert.ok(
        left.x + left.width <= right.x && right.x + right.width <= width,
      );
      await stop.click();
      await stop.waitFor({ state: "detached" });
      assert.deepEqual(await bounds(), ready);
      await input.fill("다음 질문");
      await input.press("Enter");
      await stop.click();
      const { sent, cancelled } = await page.evaluate(() => ({
        sent: window.sent,
        cancelled: window.cancelled,
      }));
      assert.equal(new Set(sent).size, 2);
      assert.deepEqual(cancelled, sent);
      await page.close();
    }
  } finally {
    await browser.close();
  }
});
