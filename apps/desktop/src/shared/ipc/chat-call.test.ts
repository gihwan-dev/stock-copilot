import { chatSessions, createChatSession } from "./chat-sessions";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { parseFreeText, parseTicker } from "../lib";
import {
  mainChatSession,
  newChatSessionId,
  newChatTurn,
  startChatTurn,
  type ChatListener,
} from "./call";
import { parseChatStreamEvent } from "./chat-schema";
import { wire } from "./schema";
import {
  answerChatSessionCommands,
  recordedMainChatTitle,
  answerChatTurnStreaming,
  installChannelWindow,
  resetCommands,
  returnDifferentChatSession,
} from "./testing/fakes";

beforeEach(installChannelWindow);
afterEach(resetCommands);

const message = parseFreeText("hello");
const symbol = parseTicker("005930");

test("startChatTurn streams typed chunks and settles with the turn status", async () => {
  answerChatTurnStreaming("without-context");
  const session = mainChatSession();
  const turn = newChatTurn();
  expect(session.kind === "ok" && turn.kind === "ok").toBe(true);
  if (session.kind !== "ok" || turn.kind !== "ok" || message.kind !== "ok")
    return;
  const listener = vi.fn<ChatListener>();
  const handle = startChatTurn(
    {
      session: session.value,
      turn: turn.value,
      message: message.value,
      context: undefined,
    },
    listener,
  );
  expect(handle.kind).toBe("ok");
  if (handle.kind !== "ok") return;
  await expect(handle.value.settled).resolves.toStrictEqual({
    kind: "ok",
    value: { status: "completed" },
  });
  expect(
    listener.mock.calls.map(([event]) =>
      event.kind === "ok" ? event.value.chunk : undefined,
    ),
  ).toStrictEqual([
    { kind: "content", content: { kind: "text", text: "holdings " } },
    {
      kind: "content",
      content: { kind: "text", text: "[article](https://news.example.test/a)" },
    },
    {
      kind: "content",
      content: { kind: "card", card: { kind: "signal", id: "signal-1" } },
    },
    {
      kind: "content",
      content: {
        kind: "sources",
        items: [
          {
            kind: "news",
            reference: "news-1",
            url: "https://news.example.test/a",
          },
        ],
      },
    },
  ]);
});

test("startChatTurn sends the screen context the panel is showing", async () => {
  answerChatTurnStreaming("with-context");
  const session = mainChatSession();
  const turn = newChatTurn();
  if (
    session.kind !== "ok" ||
    turn.kind !== "ok" ||
    message.kind !== "ok" ||
    symbol.kind !== "ok"
  )
    return expect.unreachable();
  const handle = startChatTurn(
    {
      session: session.value,
      turn: turn.value,
      message: message.value,
      context: {
        symbol: symbol.value,
        market: "kr",
        horizon: "weeks-to-months",
      },
    },
    () => undefined,
  );
  if (handle.kind !== "ok") return expect.unreachable();
  await expect(handle.value.settled).resolves.toStrictEqual({
    kind: "ok",
    value: { status: "completed" },
  });
});

test("tool names become user topics and never leak internal names", () => {
  const known = parseChatStreamEvent(
    wire({
      sequence: 1,
      turn_id: "turn-1",
      chunk: {
        ToolCallStarted: { server: "stock-copilot", tool: "get_quote" },
      },
    }),
  );
  const unknown = parseChatStreamEvent(
    wire({
      sequence: 2,
      turn_id: "turn-1",
      chunk: { ToolCallStarted: { server: "x", tool: "mystery_tool" } },
    }),
  );
  expect(known).toStrictEqual({
    kind: "ok",
    value: {
      sequence: 1,
      turn: "turn-1",
      chunk: {
        kind: "content",
        content: { kind: "tool-started", topic: "quote" },
      },
    },
  });
  expect(unknown.kind === "ok" ? unknown.value.chunk : undefined).toStrictEqual(
    { kind: "content", content: { kind: "tool-started", topic: "other" } },
  );
});

test("end chunks, failures and unsafe source links are mapped or rejected", () => {
  const failed = parseChatStreamEvent(
    wire({
      sequence: 3,
      turn_id: "turn-1",
      chunk: { Failed: { reason: "ForbiddenToolAttempt" } },
    }),
  );
  const cancelled = parseChatStreamEvent(
    wire({
      sequence: 4,
      turn_id: "turn-1",
      chunk: "Cancelled",
    }),
  );
  const insecure = parseChatStreamEvent(
    wire({
      sequence: 5,
      turn_id: "turn-1",
      chunk: {
        Sources: {
          items: [{ kind: "Web", reference: "w", url: "http://plain.test/" }],
        },
      },
    }),
  );
  const broken = parseChatStreamEvent(
    wire({
      sequence: 6,
      turn_id: "turn-1",
      chunk: { Html: "<b>x</b>" },
    }),
  );
  expect(failed.kind === "ok" ? failed.value.chunk : undefined).toStrictEqual({
    kind: "end",
    end: { kind: "failed", reason: "forbidden-tool" },
  });
  expect(
    cancelled.kind === "ok" ? cancelled.value.chunk : undefined,
  ).toStrictEqual({ kind: "end", end: { kind: "cancelled" } });
  expect(
    insecure.kind === "ok" ? insecure.value.chunk : undefined,
  ).toStrictEqual({
    kind: "content",
    content: {
      kind: "sources",
      items: [{ kind: "web", reference: "w", url: undefined }],
    },
  });
  expect(broken).toStrictEqual({
    kind: "err",
    error: { kind: "invalid-response" },
  });
});

test("session commands preserve titles, empty chats and the requested identity", async () => {
  answerChatSessionCommands();
  const session = newChatSessionId();
  expect(session.kind).toBe("ok");
  if (session.kind !== "ok") return;
  await expect(chatSessions()).resolves.toStrictEqual({
    kind: "ok",
    value: [
      { session: "main-chat", title: recordedMainChatTitle },
      { session: "session-new", title: undefined },
    ],
  });
  await expect(createChatSession(session.value)).resolves.toStrictEqual({
    kind: "ok",
    value: { session: session.value, title: undefined },
  });
  returnDifferentChatSession();
  await expect(createChatSession(session.value)).resolves.toStrictEqual({
    kind: "err",
    error: { kind: "invalid-response" },
  });
});
