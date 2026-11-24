import assert from "node:assert/strict";
import { join } from "node:path";
import test from "node:test";
import { desktopRoot } from "./paths.mjs";
import { screenDom } from "./screen-dom.mjs";

const panelEntry = JSON.stringify(
  join(desktopRoot, "src/features/chat/index.ts"),
);

const emptyHistory = {
  session_id: "main-chat",
  entries: [],
  active_turn: null,
};

const turnResponse = (request, status) => ({
  session_id: request.session_id,
  turn_id: request.turn_id,
  run_id: request.run_id,
  status,
  mode: "FreshAnalysis",
  actual_model: null,
  first_token_latency_ms: null,
  resets_at_unix_seconds: null,
  failure_reason: null,
});

const streamedAnswer = (request, onChunk) => {
  const send = (sequence, chunk) =>
    onChunk.onmessage({ sequence, turn_id: request.turn_id, chunk });
  send(1, {
    ToolCallStarted: { server: "stock-copilot", tool: "get_portfolio" },
  });
  send(2, {
    ToolCallFinished: {
      server: "stock-copilot",
      tool: "get_portfolio",
      status: "Succeeded",
    },
  });
  send(3, {
    TextDelta: {
      text: "보유 종목 요약 [기사](https://news.example.test/a) [새는 주소](https://evil.example.test/?q=005930) <b>굵게</b> ![그림](https://news.example.test/a.png)",
    },
  });
  send(4, { Card: { kind: "Signal", id: "signal-1" } });
  send(5, { Card: { kind: "Holding", id: "holding-1" } });
  send(6, {
    Sources: {
      items: [
        {
          kind: "News",
          reference: "news-1",
          url: "https://news.example.test/a",
        },
        { kind: "Web", reference: "web-1", url: null },
      ],
    },
  });
  send(7, {
    Done: {
      session_id: request.session_id,
      turn_id: request.turn_id,
      run_id: request.run_id,
      mode: "FreshAnalysis",
      actual_model: null,
      first_token_latency_ms: 10,
      finished_at_unix_seconds: "1",
    },
  });
  return turnResponse(request, "Completed");
};

const mountSource = `
  import { createElement as h } from "react";
  import { QueryClientProvider } from "@tanstack/react-query";
  import { render, fireEvent, waitFor, cleanup } from "@testing-library/react";
  import { clearMocks } from "@tauri-apps/api/mocks";
  import { createAppQueryClient } from QUERY_ENTRY;
  import { mockCommands } from MOCK_IPC_ENTRY;
  import { parseTicker } from LIB_ENTRY;
  import { ChatPanel } from ${panelEntry};
  window.cleanup = cleanup;
  window.clearMocks = clearMocks;
  window.mockCommands = mockCommands;
  window.waitFor = waitFor;
  window.fireEvent = fireEvent;
  window.mountPanel = (withContext) => {
    const ticker = parseTicker("005930");
    const context = withContext && ticker.kind === "ok"
      ? { symbol: ticker.value, market: "kr", horizon: "weeks-to-months" }
      : undefined;
    return render(
      h(QueryClientProvider, { client: createAppQueryClient() },
        h(ChatPanel, { context })),
    );
  };
`;

const until = async (w, check, label) =>
  w.waitFor(() => {
    if (!check()) throw new Error(`waiting for ${label}`);
  });

const send = (w, view, text) => {
  const box = view.container.querySelector("textarea");
  w.fireEvent.change(box, { target: { value: text } });
  const button = Array.from(view.container.querySelectorAll("button")).find(
    (b) => b.textContent === "보내기",
  );
  w.fireEvent.click(button);
};

test("chat panel streams a reply with cards, sources and only collected links", async () => {
  const bundle = await screenDom(mountSource);
  try {
    const w = bundle.open();
    const requests = [];
    const opened = [];
    w.mockCommands({
      getChatHistory: () => emptyHistory,
      sendChatMessage: ({ request, onChunk }) => {
        requests.push(request);
        return streamedAnswer(request, onChunk);
      },
      openExternal: ({ request }) => {
        opened.push(request.url);
        return { status: "opened" };
      },
    });
    const view = w.mountPanel(true);
    await until(
      w,
      () => view.container.textContent.includes("무엇이든 물어보세요"),
      "empty state",
    );
    send(w, view, "내 보유 종목 어때?");
    await until(
      w,
      () => view.container.textContent.includes("보유 종목 요약"),
      "reply",
    );
    const content = view.container.textContent;
    assert.equal(requests.length, 1);
    assert.equal(requests[0].message, "내 보유 종목 어때?");
    assert.deepEqual(JSON.parse(JSON.stringify(requests[0].context)), {
      symbol: "005930",
      market: "Kr",
      horizon: "WeeksToMonths",
    });
    assert.ok(!content.includes("get_portfolio"));
    assert.ok(!content.includes("stock-copilot"));
    assert.ok(!view.container.querySelector("b"));
    assert.ok(!view.container.querySelector("img"));
    assert.ok(!view.container.querySelector("a"));
    assert.ok(content.includes("신호"));
    assert.ok(content.includes("보유 종목"));
    assert.ok(content.includes("뉴스"));
    assert.ok(content.includes("미검증 웹 값"));
    assert.ok(!content.includes("signal-1"));
    const approve = Array.from(view.container.querySelectorAll("button")).find(
      (b) => b.textContent === "주문 승인",
    );
    assert.ok(approve);
    assert.equal(approve.disabled, true);
    const buttons = Array.from(view.container.querySelectorAll("button"));
    const article = buttons.find((b) => b.textContent === "기사");
    assert.ok(article);
    assert.equal(
      buttons.find((b) => b.textContent === "새는 주소"),
      undefined,
    );
    assert.equal(
      buttons.find((b) => b.textContent === "그림"),
      undefined,
    );
    w.fireEvent.click(article);
    await until(w, () => opened.length === 1, "open external");
    assert.deepEqual(opened, ["https://news.example.test/a"]);
  } finally {
    bundle.close();
  }
});

test("chat panel shows the lookup in user words while a tool runs and can stop", async () => {
  const bundle = await screenDom(mountSource);
  try {
    const w = bundle.open();
    let pending;
    const cancelled = [];
    w.mockCommands({
      getChatHistory: () => emptyHistory,
      sendChatMessage: ({ request, onChunk }) => {
        onChunk.onmessage({
          sequence: 1,
          turn_id: request.turn_id,
          chunk: {
            ToolCallStarted: { server: "stock-copilot", tool: "get_quote" },
          },
        });
        return new Promise((resolve) => {
          pending = () => {
            onChunk.onmessage({
              sequence: 2,
              turn_id: request.turn_id,
              chunk: "Cancelled",
            });
            resolve(turnResponse(request, "Cancelled"));
          };
        });
      },
      cancelChatTurn: ({ request }) => {
        cancelled.push(request.turn_id);
        pending();
        return { turn_id: request.turn_id, outcome: "Signaled" };
      },
    });
    const view = w.mountPanel(false);
    await until(w, () => view.container.querySelector("textarea"), "input");
    send(w, view, "삼성전자 지금 얼마야?");
    await until(
      w,
      () => view.container.textContent.includes("현재가를 확인하는 중이에요"),
      "lookup",
    );
    assert.ok(!view.container.textContent.includes("get_quote"));
    const stop = Array.from(view.container.querySelectorAll("button")).find(
      (b) => b.textContent === "멈추기",
    );
    assert.ok(stop);
    w.fireEvent.click(stop);
    await until(
      w,
      () => view.container.textContent.includes("답을 멈췄어요."),
      "cancelled",
    );
    assert.equal(cancelled.length, 1);
  } finally {
    bundle.close();
  }
});

test("chat panel shows failures and history load errors in plain words", async () => {
  const bundle = await screenDom(mountSource);
  try {
    const w = bundle.open();
    w.mockCommands({
      getChatHistory: () => emptyHistory,
      sendChatMessage: ({ request, onChunk }) => {
        onChunk.onmessage({
          sequence: 1,
          turn_id: request.turn_id,
          chunk: { Failed: { reason: "ForbiddenToolAttempt" } },
        });
        return turnResponse(request, "Failed");
      },
    });
    const view = w.mountPanel(false);
    await until(w, () => view.container.querySelector("textarea"), "input");
    send(w, view, "주문 넣어 줘");
    await until(
      w,
      () => view.container.textContent.includes("안전 점검을 통과하지 못해"),
      "failed",
    );
    w.cleanup();
    w.mockCommands({
      getChatHistory: () => {
        throw { kind: "retryable" };
      },
    });
    const broken = w.mountPanel(false);
    await until(
      w,
      () =>
        broken.container.textContent.includes("지난 대화를 불러오지 못했어요."),
      "history error",
    );
  } finally {
    bundle.close();
  }
});

test("chat panel replays stored turns and resubscribes to a running one", async () => {
  const bundle = await screenDom(mountSource);
  try {
    const w = bundle.open();
    const resubscribed = [];
    w.mockCommands({
      getChatHistory: () => ({
        session_id: "main-chat",
        entries: [
          {
            sequence: 1,
            at_unix_seconds: "1",
            turn_id: "old",
            role: "User",
            text: "어제 질문",
            status: null,
            cards: [],
            sources: [],
          },
          {
            sequence: 2,
            at_unix_seconds: "2",
            turn_id: "old",
            role: "Assistant",
            text: "어제 답",
            status: "Completed",
            cards: [],
            sources: [],
          },
          {
            sequence: 3,
            at_unix_seconds: "3",
            turn_id: "live",
            role: "User",
            text: "지금 질문",
            status: null,
            cards: [],
            sources: [],
          },
        ],
        active_turn: { turn_id: "live", last_sequence: 1, state: "Running" },
      }),
      resubscribeChatTurn: ({ request, onChunk }) => {
        resubscribed.push(request);
        onChunk.onmessage({
          sequence: 1,
          turn_id: "live",
          chunk: { TextDelta: { text: "이어진 답" } },
        });
        onChunk.onmessage({
          sequence: 2,
          turn_id: "live",
          chunk: "Interrupted",
        });
        return { turn_id: "live", replayed_count: 2, state: "Finished" };
      },
    });
    const view = w.mountPanel(false);
    await until(
      w,
      () => view.container.textContent.includes("이어진 답"),
      "replayed",
    );
    const content = view.container.textContent;
    assert.ok(content.includes("어제 질문"));
    assert.ok(content.includes("어제 답"));
    assert.ok(content.includes("답이 중간에 끊겼어요"));
    assert.deepEqual(JSON.parse(JSON.stringify(resubscribed)), [
      { turn_id: "live", after_sequence: 0 },
    ]);
  } finally {
    bundle.close();
  }
});

test("chat panel marks a finished reply without sources as having no source", async () => {
  const bundle = await screenDom(mountSource);
  try {
    const w = bundle.open();
    w.mockCommands({
      getChatHistory: () => emptyHistory,
      sendChatMessage: ({ request, onChunk }) => {
        onChunk.onmessage({
          sequence: 1,
          turn_id: request.turn_id,
          chunk: { TextDelta: { text: "근거 없는 답" } },
        });
        return turnResponse(request, "Completed");
      },
    });
    const view = w.mountPanel(false);
    await until(w, () => view.container.querySelector("textarea"), "input");
    send(w, view, "아무 말");
    await until(
      w,
      () => view.container.textContent.includes("출처 없음"),
      "no source",
    );
    assert.ok(view.container.textContent.includes("근거 없는 답"));
  } finally {
    bundle.close();
  }
});
