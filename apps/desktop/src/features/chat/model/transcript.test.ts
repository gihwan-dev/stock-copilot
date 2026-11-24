import { expect, test } from "vitest";
import type { ChatStreamChunk, ChatStreamEvent } from "../../../shared/ipc";
import {
  parseCount,
  parseEntityId,
  parseExternalUrl,
  parseFreeText,
  type Count,
  type EntityId,
  type ExternalUrl,
  type FreeText,
} from "../../../shared/lib";
import {
  applyEvent,
  beginExchange,
  emptyTranscript,
  fromHistory,
  resumableTurn,
  settleExchange,
  type Transcript,
} from "./transcript";
import { linkVerdict, panelState, streamingTurn } from "./view";

const id = (raw: "turn-1" | "old" | "lost"): EntityId => {
  const parsed = parseEntityId(raw);
  return parsed.kind === "ok" ? parsed.value : expect.unreachable();
};
const words = (
  raw: "q" | "a" | "b" | "late" | "n" | "first" | "answer" | "second" | "third",
): FreeText => {
  const parsed = parseFreeText(raw);
  return parsed.kind === "ok" ? parsed.value : expect.unreachable();
};
const seq = (raw: 1 | 2 | 3 | 4): Count => {
  const parsed = parseCount(raw);
  return parsed.kind === "ok" ? parsed.value : expect.unreachable();
};
const link = (
  raw:
    | "https://news.example.test/a"
    | "https://evil.example.test/?holdings=005930",
): ExternalUrl => {
  const parsed = parseExternalUrl(raw);
  return parsed.kind === "ok" ? parsed.value : expect.unreachable();
};

const turn = id("turn-1");
const event = (
  sequence: 1 | 2 | 3 | 4,
  chunk: ChatStreamChunk,
): ChatStreamEvent => ({
  sequence: seq(sequence),
  turn,
  chunk,
});

const replay = (
  start: Transcript,
  events: readonly ChatStreamEvent[],
): Transcript => events.reduce(applyEvent, start);

const started = beginExchange(emptyTranscript, turn, words("q"));

test("the panel moves through empty, looking-up, streaming and failed states", () => {
  expect(panelState(emptyTranscript)).toBe("empty");
  expect(panelState(started)).toBe("streaming");
  const lookingUp = applyEvent(
    started,
    event(1, {
      kind: "content",
      content: { kind: "tool-started", topic: "portfolio" },
    }),
  );
  expect(panelState(lookingUp)).toBe("looking-up");
  expect(lookingUp.exchanges[0]?.reply.tool).toBe("portfolio");
  const failed = applyEvent(
    lookingUp,
    event(2, { kind: "end", end: { kind: "failed", reason: "connection" } }),
  );
  expect(panelState(failed)).toBe("failed");
  expect(failed.exchanges[0]?.reply.tool).toBeUndefined();
  expect(streamingTurn(failed)).toBeUndefined();
});

test("text deltas append in order, replays are ignored and done ends the reply", () => {
  const text = (value: "a" | "b" | "late"): ChatStreamChunk => ({
    kind: "content",
    content: { kind: "text", text: words(value) },
  });
  const done = replay(started, [
    event(1, text("a")),
    event(2, text("b")),
    event(1, text("a")),
    event(3, { kind: "end", end: { kind: "done" } }),
    event(4, text("late")),
  ]);
  expect(done.exchanges[0]?.reply.text).toBe("ab");
  expect(done.exchanges[0]?.reply.phase).toBe("done");
  expect(panelState(done)).toBe("idle");
});

test("only source urls of the same reply become links", () => {
  const withSources = applyEvent(
    started,
    event(1, {
      kind: "content",
      content: {
        kind: "sources",
        items: [
          {
            kind: "news",
            reference: words("n"),
            url: link("https://news.example.test/a"),
          },
        ],
      },
    }),
  );
  const reply = withSources.exchanges[0]?.reply;
  if (reply === undefined) return expect.unreachable();
  expect(linkVerdict(reply, link("https://news.example.test/a"))).toBe("allow");
  expect(
    linkVerdict(reply, link("https://evil.example.test/?holdings=005930")),
  ).toBe("deny");
});

test("web research shows only that it was looked up", () => {
  const researched = applyEvent(
    started,
    event(1, {
      kind: "content",
      content: {
        kind: "tool-finished",
        topic: "web-research",
        status: "succeeded",
      },
    }),
  );
  expect(researched.exchanges[0]?.reply.researched).toBe("researched");
});

test("settling closes a still-streaming reply and maps lost connections", () => {
  expect(
    settleExchange(started, turn, "completed").exchanges[0]?.reply.phase,
  ).toBe("done");
  const lost = settleExchange(started, turn, "unreachable").exchanges[0]?.reply;
  expect(lost?.phase).toBe("failed");
  expect(lost?.failure).toBe("connection");
});

test("history rebuilds exchanges and keeps only a running turn resumable", () => {
  const history = {
    entries: [
      {
        turn: id("old"),
        role: "user" as const,
        text: words("first"),
        status: undefined,
        cards: [],
        sources: [],
      },
      {
        turn: id("old"),
        role: "assistant" as const,
        text: words("answer"),
        status: "completed" as const,
        cards: [],
        sources: [],
      },
      {
        turn: id("lost"),
        role: "user" as const,
        text: words("second"),
        status: undefined,
        cards: [],
        sources: [],
      },
      {
        turn,
        role: "user" as const,
        text: words("third"),
        status: undefined,
        cards: [],
        sources: [],
      },
    ],
    active: { turn, state: "running" as const },
  };
  const rebuilt = fromHistory(history);
  expect(
    rebuilt.exchanges.map((exchange) => exchange.reply.phase),
  ).toStrictEqual(["done", "interrupted", "streaming"]);
  expect(resumableTurn(history)).toBe(turn);
  expect(
    resumableTurn({ ...history, active: { turn, state: "finished" } }),
  ).toBeUndefined();
});
