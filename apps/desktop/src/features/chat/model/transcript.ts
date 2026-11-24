import type {
  ChatCardRef,
  ChatContentChunk,
  ChatEndChunk,
  ChatFailureReason,
  ChatHistory,
  ChatHistoryEntry,
  ChatNumberCheck,
  ChatSource,
  ChatStreamChunk,
  ChatStreamEvent,
  ChatToolStatus,
  ChatToolTopic,
  ChatTurnStatus,
} from "../../../shared/ipc";
import {
  appendFreeText,
  emptyFreeText,
  type Count,
  type EntityId,
  type FreeText,
} from "../../../shared/lib";

export type ReplyPhase =
  | "streaming"
  | "done"
  | "cancelled"
  | "interrupted"
  | "rate-limited"
  | "failed";

export type AssistantReply = {
  readonly text: FreeText;
  readonly phase: ReplyPhase;
  readonly failure: ChatFailureReason | undefined;
  readonly tool: ChatToolTopic | undefined;
  readonly cards: readonly ChatCardRef[];
  readonly sources: readonly ChatSource[];
  readonly seen: Count | undefined;
  readonly researched: "researched" | "none";
  readonly numbers: ChatNumberCheck | "unchecked";
};

export type ChatExchange = {
  readonly turn: EntityId;
  readonly question: FreeText;
  readonly reply: AssistantReply;
};

export type Transcript = { readonly exchanges: readonly ChatExchange[] };

export type PanelState =
  "empty" | "looking-up" | "streaming" | "failed" | "idle";

export const emptyTranscript: Transcript = { exchanges: [] };

const freshReply: AssistantReply = {
  text: emptyFreeText,
  phase: "streaming",
  failure: undefined,
  tool: undefined,
  cards: [],
  sources: [],
  seen: undefined,
  researched: "none",
  numbers: "unchecked",
};

const phaseOf = (status: ChatTurnStatus | undefined): ReplyPhase => {
  switch (status) {
    case "completed":
      return "done";
    case "cancelled":
      return "cancelled";
    case "interrupted":
      return "interrupted";
    case "rate-limited":
      return "rate-limited";
    case "failed":
      return "failed";
    case "in-progress":
    case undefined:
      return "streaming";
  }
};

const stopped = (reply: AssistantReply, phase: ReplyPhase): AssistantReply => ({
  ...reply,
  tool: undefined,
  phase,
});

const withEntry = (
  exchanges: readonly ChatExchange[],
  entry: ChatHistoryEntry,
): readonly ChatExchange[] => {
  const known = exchanges.some((exchange) => exchange.turn === entry.turn);
  if (entry.role === "user")
    return known
      ? exchanges
      : [
          ...exchanges,
          { turn: entry.turn, question: entry.text, reply: freshReply },
        ];
  if (entry.status === "in-progress") return exchanges;
  const reply: AssistantReply = {
    ...freshReply,
    text: entry.text,
    phase: phaseOf(entry.status),
    cards: entry.cards,
    sources: entry.sources,
  };
  return known
    ? exchanges.map((exchange) =>
        exchange.turn === entry.turn ? { ...exchange, reply } : exchange,
      )
    : [...exchanges, { turn: entry.turn, question: emptyFreeText, reply }];
};

export const resumableTurn = (history: ChatHistory): EntityId | undefined =>
  history.active?.state === "running" ? history.active.turn : undefined;

export const fromHistory = (history: ChatHistory): Transcript => ({
  exchanges: history.entries
    .reduce<readonly ChatExchange[]>(withEntry, [])
    .map((exchange) =>
      exchange.reply.phase !== "streaming" ||
      exchange.turn === resumableTurn(history)
        ? exchange
        : { ...exchange, reply: stopped(exchange.reply, "interrupted") },
    ),
});

export const beginExchange = (
  transcript: Transcript,
  turn: EntityId,
  question: FreeText,
): Transcript => ({
  exchanges: [...transcript.exchanges, { turn, question, reply: freshReply }],
});

const finishTool = (
  reply: AssistantReply,
  topic: ChatToolTopic,
  status: ChatToolStatus,
): AssistantReply => ({
  ...reply,
  tool: undefined,
  researched:
    topic === "web-research" && status === "succeeded"
      ? "researched"
      : reply.researched,
});

const applyContent = (
  reply: AssistantReply,
  chunk: ChatContentChunk,
): AssistantReply => {
  switch (chunk.kind) {
    case "text":
      return { ...reply, text: appendFreeText(reply.text, chunk.text) };
    case "tool-started":
      return { ...reply, tool: chunk.topic };
    case "tool-finished":
      return finishTool(reply, chunk.topic, chunk.status);
    case "card":
      return { ...reply, cards: [...reply.cards, chunk.card] };
    case "sources":
      return { ...reply, sources: [...reply.sources, ...chunk.items] };
    case "number-check":
      return { ...reply, numbers: chunk.check };
  }
};

const applyEnd = (
  reply: AssistantReply,
  chunk: ChatEndChunk,
): AssistantReply =>
  chunk.kind === "failed"
    ? { ...stopped(reply, "failed"), failure: chunk.reason }
    : stopped(reply, chunk.kind);

const applyChunk = (
  reply: AssistantReply,
  chunk: ChatStreamChunk,
): AssistantReply =>
  chunk.kind === "content"
    ? applyContent(reply, chunk.content)
    : applyEnd(reply, chunk.end);

const freshness = (reply: AssistantReply, sequence: Count): "fresh" | "seen" =>
  reply.seen !== undefined && sequence <= reply.seen ? "seen" : "fresh";

const accepts = (
  exchange: ChatExchange,
  event: ChatStreamEvent,
): "accept" | "skip" =>
  exchange.turn === event.turn &&
  exchange.reply.phase === "streaming" &&
  freshness(exchange.reply, event.sequence) === "fresh"
    ? "accept"
    : "skip";

export const applyEvent = (
  transcript: Transcript,
  event: ChatStreamEvent,
): Transcript => ({
  exchanges: transcript.exchanges.map((exchange) =>
    accepts(exchange, event) === "skip"
      ? exchange
      : {
          ...exchange,
          reply: {
            ...applyChunk(exchange.reply, event.chunk),
            seen: event.sequence,
          },
        },
  ),
});

const settledReply = (
  reply: AssistantReply,
  status: ChatTurnStatus | "unreachable",
): AssistantReply => {
  if (status === "unreachable")
    return { ...stopped(reply, "failed"), failure: "connection" };
  const phase = phaseOf(status);
  return stopped(reply, phase === "streaming" ? "interrupted" : phase);
};

export const settleExchange = (
  transcript: Transcript,
  turn: EntityId,
  status: ChatTurnStatus | "unreachable",
): Transcript => ({
  exchanges: transcript.exchanges.map((exchange) =>
    exchange.turn !== turn || exchange.reply.phase !== "streaming"
      ? exchange
      : { ...exchange, reply: settledReply(exchange.reply, status) },
  ),
});
