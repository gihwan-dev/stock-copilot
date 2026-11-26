import assert from "node:assert/strict";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

let fixture;
before(async () => {
  fixture = await componentsDom(String.raw`
import {createElement as h} from "react";
import {render,cleanup,act,fireEvent} from "@testing-library/react";
import {AppShell,Markdown,ChatInput,Text} from DS_ENTRY;
import {parseFreeText,text} from LIB_ENTRY;
window.cleanup=cleanup;window.act=act;window.fire=fireEvent;
window.markdown=(value,flavor)=>{cleanup();window.calls=[];return render(h(Markdown,{content:parseFreeText(value).value,flavor,linkAllowed:()=>"allow",onOpenLink:url=>window.calls.push(url)}));};
window.composer=(draft)=>{cleanup();window.calls=[];window.drafts=[];return render(h(ChatInput,{layout:"compact",onSend:value=>window.calls.push(value),onDraftChange:value=>window.drafts.push(value),...(draft===undefined?{}:{draft:parseFreeText(draft).value})}));};
window.shell=()=>{cleanup();return render(h(AppShell,{
  chat:h(Text,null,text("shell.chat","ko")),
  chatComposer:h(Text,null,text("chat.send","ko")),
  chatNotice:{summary:text("notice.chatDataShort","ko"),detail:text("notice.chatData","ko")},
},h(Text,null,text("app.title","ko"))));};
`);
});
after(() => fixture?.close());

test("chat markdown renders GFM tables, code, lists and strikethrough without raw HTML", async () => {
  const w = fixture.open();
  const value =
    "## 요약\n\n- 하나\n- 둘\n\n1. 첫째\n2. 둘째\n\n| 종목 | 비중 |\n| --- | ---: |\n| 삼성전자 | 42% |\n\n```ts\nconst 길다 = 1;\n```\n\n문장 `코드` ~~취소~~\n\n<b>굵게</b>\n\n> 인용";
  await w.act(() => w.markdown(value, "gfm"));
  const doc = w.document;
  assert.equal(doc.querySelectorAll("table").length, 1);
  assert.equal(doc.querySelectorAll("th").length, 2);
  assert.ok(doc.querySelector("td").textContent.includes("삼성전자"));
  assert.ok(doc.querySelector("table").closest("[data-scroll-viewport]"));
  assert.ok(doc.querySelector("pre").classList.contains("font-mono"));
  assert.ok(doc.querySelector("pre").classList.contains("chat-content"));
  assert.ok(doc.querySelector("p code").classList.contains("font-mono"));
  assert.ok(doc.querySelector("ul").classList.contains("markdown-bullets"));
  assert.ok(doc.querySelector("ol").classList.contains("markdown-ordered"));
  assert.ok(doc.querySelector("blockquote"));
  assert.ok(doc.querySelector("del"));
  assert.equal(doc.querySelectorAll("b,script,a,img").length, 0);
});

test("chat markdown keeps partial streaming input readable", async () => {
  const w = fixture.open();
  for (const value of [
    "| 종목 | 비중",
    "| 종목 | 비중 |\n| --- |",
    "```ts\nconst a",
    "**굵",
    "- [ ] 할 일\n- [x] 끝",
  ]) {
    await w.act(() => w.markdown(value, "gfm"));
    assert.equal(w.document.querySelectorAll("input,a").length, 0, value);
  }
});

test("chat markdown links and footnotes open only through the external callback", async () => {
  const w = fixture.open();
  await w.act(() =>
    w.markdown(
      "[기사](https://news.example.test/a) https://bare.example.test/b 설명[^1]\n\n[^1]: 각주",
      "gfm",
    ),
  );
  const doc = w.document;
  assert.equal(doc.querySelectorAll("a,[href]").length, 0);
  assert.equal(doc.body.textContent.includes("Footnotes"), false);
  const buttons = Array.from(doc.querySelectorAll("button"));
  assert.deepEqual(
    buttons.map((b) => b.textContent),
    ["기사", "https://bare.example.test/b"],
  );
  for (const button of buttons) await w.act(() => w.fire.click(button));
  assert.deepEqual(Array.from(w.calls), [
    "https://news.example.test/a",
    "https://bare.example.test/b",
  ]);
});

test("basic markdown keeps the previous no-table behavior", async () => {
  const w = fixture.open();
  await w.act(() =>
    w.markdown("| 이름 | 값 |\n| --- | --- |\n| 예시 | 1 |", undefined),
  );
  assert.equal(w.document.querySelectorAll("table").length, 0);
});

test("compact composer sends on Enter, keeps Shift+Enter, and follows a controlled draft", async () => {
  const w = fixture.open();
  await w.act(() => w.composer());
  const box = w.document.querySelector("textarea");
  assert.equal(box.getAttribute("aria-label"), "메시지");
  assert.ok(box.getAttribute("placeholder"));
  assert.equal(w.document.querySelector("label"), null);
  const button = w.document.querySelector("button");
  assert.equal(button.textContent, "보내기");
  assert.equal(button.disabled, true);
  await w.act(() => w.fire.change(box, { target: { value: "첫 줄" } }));
  await w.act(() => w.fire.keyDown(box, { key: "Enter", shiftKey: true }));
  assert.equal(w.calls.length, 0);
  await w.act(() => w.fire.keyDown(box, { key: "Enter" }));
  assert.deepEqual(Array.from(w.calls), ["첫 줄"]);
  assert.equal(box.value, "");
  assert.deepEqual(Array.from(w.drafts), ["첫 줄", ""]);
  await w.act(() => w.composer("추천 질문"));
  assert.equal(w.document.querySelector("textarea").value, "추천 질문");
  assert.equal(w.document.querySelector("button").disabled, false);
});

test("chat panel pins the composer below the scrolling messages and folds the notice", async () => {
  const w = fixture.open();
  const view = await w.act(() => w.shell());
  const aside = w.document.querySelector('aside[aria-label="AI 채팅"]');
  assert.ok(aside);
  const composer = Array.from(aside.querySelectorAll("p")).find(
    (p) => p.textContent === "보내기",
  );
  assert.ok(composer);
  assert.equal(composer.closest("[data-scroll-viewport]"), null);
  const message = Array.from(aside.querySelectorAll("p")).find(
    (p) => p.textContent === "AI 채팅" && p.closest("[data-scroll-viewport]"),
  );
  assert.ok(message);
  assert.ok(
    message.compareDocumentPosition(composer) &
      w.Node.DOCUMENT_POSITION_FOLLOWING,
  );
  const detail = Array.from(aside.querySelectorAll("p")).find((p) =>
    p.textContent.startsWith("AI를 쓰면"),
  );
  assert.equal(detail.hidden, true);
  const toggle = aside.querySelector(
    'button[aria-label="전송 안내 자세히 보기"]',
  );
  assert.equal(toggle.getAttribute("aria-expanded"), "false");
  await w.act(() => w.fire.click(toggle));
  assert.equal(detail.hidden, false);
  assert.ok(
    aside.querySelector(
      'button[aria-label="전송 안내 접기"][aria-expanded="true"]',
    ),
  );
  view.unmount();
});

const panelEntry = JSON.stringify(
  join(desktopRoot, "src/features/chat/index.ts"),
);

test("empty chat suggestions fill the composer without sending", async () => {
  const bundle = await screenDom(`
    import { createElement as h } from "react";
    import { QueryClientProvider } from "@tanstack/react-query";
    import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
    import { clearMocks } from "@tauri-apps/api/mocks";
    import { createAppQueryClient } from QUERY_ENTRY;
    import { mockCommands } from MOCK_IPC_ENTRY;
    import { ChatPanel } from ${panelEntry};
    window.cleanup = cleanup;
    window.clearMocks = clearMocks;
    window.mockCommands = mockCommands;
    window.waitFor = waitFor;
    window.fireEvent = fireEvent;
    window.mountPanel = () =>
      render(h(QueryClientProvider, { client: createAppQueryClient() }, h(ChatPanel, { context: undefined })));
  `);
  try {
    const w = bundle.open();
    const sent = [];
    w.mockCommands({
      getChatHistory: () => ({
        session_id: "main-chat",
        entries: [],
        active_turn: null,
      }),
      sendChatMessage: ({ request }) => {
        sent.push(request.message);
        return {
          session_id: request.session_id,
          turn_id: request.turn_id,
          run_id: request.run_id,
          status: "Completed",
          mode: "FreshAnalysis",
          actual_model: null,
          first_token_latency_ms: null,
          resets_at_unix_seconds: null,
          failure_reason: null,
        };
      },
    });
    const view = w.mountPanel();
    await w.waitFor(() => {
      if (!view.container.textContent.includes("무엇이든 물어보세요"))
        throw new Error("waiting for empty state");
    });
    const suggestion = Array.from(
      view.container.querySelectorAll("button"),
    ).find((b) => b.textContent === "내 보유 종목 지금 어때?");
    assert.ok(suggestion);
    w.fireEvent.click(suggestion);
    const box = view.container.querySelector("textarea");
    assert.equal(box.value, "내 보유 종목 지금 어때?");
    assert.deepEqual(sent, []);
    const send = Array.from(view.container.querySelectorAll("button")).find(
      (b) => b.textContent === "보내기",
    );
    w.fireEvent.click(send);
    await w.waitFor(() => {
      if (sent.length !== 1) throw new Error("waiting for send");
    });
    assert.deepEqual(sent, ["내 보유 종목 지금 어때?"]);
  } finally {
    bundle.close();
  }
});
