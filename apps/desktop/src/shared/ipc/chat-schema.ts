import { z } from "zod";
import {
  chatIdSchema,
  countSchema,
  err,
  externalUrlSchema,
  freeTextSchema,
  ok,
  type EntityId,
  type Result,
} from "../lib";
import {
  toolNames,
  toolTopics,
  toolStatuses,
  toolStatusesCodes,
  cardKinds,
  cardKindsCodes,
  sourceKinds,
  sourceKindsCodes,
  failures,
  failuresCodes,
  turnStatuses,
  turnStatusesCodes,
} from "./chat-codes";
import type {
  ChatActiveState,
  ChatCancelOutcome,
  ChatCardKind,
  ChatContentChunk,
  ChatEndChunk,
  ChatFailureReason,
  ChatHistory,
  ChatHistoryEntry,
  ChatResubscribed,
  ChatRole,
  ChatSource,
  ChatSourceKind,
  ChatStreamChunk,
  ChatStreamEvent,
  ChatToolStatus,
  ChatToolTopic,
  ChatTurnSettled,
  ChatTurnStatus,
} from "./chat-types";
import {
  settleWithSchema as settleOutcome,
  type IpcFailure,
  type Wire,
  type WireOutcome,
} from "./schema";

const idSchema = chatIdSchema;
const bodySchema = freeTextSchema;
const linkSchema = z.union([
  externalUrlSchema,
  z.unknown().transform((): undefined => undefined),
]);
const sequenceSchema = countSchema;

const topicSchema = z.union([
  z.enum(toolNames).transform((name): ChatToolTopic => toolTopics[name]),
  z.string().transform((): ChatToolTopic => "other"),
]);
const toolStatusSchema = z
  .enum(toolStatusesCodes)
  .transform((key): ChatToolStatus => toolStatuses[key]);
const cardKindSchema = z
  .enum(cardKindsCodes)
  .transform((key): ChatCardKind => cardKinds[key]);
const sourceKindSchema = z
  .enum(sourceKindsCodes)
  .transform((key): ChatSourceKind => sourceKinds[key]);
const failureSchema = z
  .enum(failuresCodes)
  .transform((key): ChatFailureReason => failures[key]);
const turnStatusSchema = z
  .enum(turnStatusesCodes)
  .transform((key): ChatTurnStatus => turnStatuses[key]);
const activeSchema = z
  .enum(["Running", "Finished"])
  .transform((key): ChatActiveState =>
    key === "Running" ? "running" : "finished",
  );
const cardSchema = z.strictObject({ kind: cardKindSchema, id: idSchema });

const sourceSchema = z
  .object({ kind: sourceKindSchema, reference: bodySchema, url: linkSchema })
  .transform((value): ChatSource => ({
    kind: value.kind,
    reference: value.reference,
    url: value.url,
  }));

const tool = z.object({ server: z.string(), tool: topicSchema });
const numberChecks = {
  NoNumbers: "no-numbers",
  Verified: "verified",
  Unverified: "unverified",
} as const;
const chunk = (content: ChatContentChunk): ChatStreamChunk => ({
  kind: "content",
  content,
});
const end = (value: ChatEndChunk): ChatStreamChunk => ({
  kind: "end",
  end: value,
});
const chunkSchema = z.union([
  z
    .strictObject({ TextDelta: z.object({ text: bodySchema }) })
    .transform((v) => chunk({ kind: "text", text: v.TextDelta.text })),
  z
    .strictObject({ ToolCallStarted: tool })
    .transform((v) =>
      chunk({ kind: "tool-started", topic: v.ToolCallStarted.tool }),
    ),
  z
    .strictObject({
      ToolCallFinished: tool.extend({ status: toolStatusSchema }),
    })
    .transform((v) =>
      chunk({
        kind: "tool-finished",
        topic: v.ToolCallFinished.tool,
        status: v.ToolCallFinished.status,
      }),
    ),
  z
    .strictObject({ Card: cardSchema })
    .transform((v) => chunk({ kind: "card", card: v.Card })),
  z
    .strictObject({ Sources: z.object({ items: z.array(sourceSchema) }) })
    .transform((v) => chunk({ kind: "sources", items: v.Sources.items })),
  z
    .strictObject({
      NumberCheck: z.object({
        status: z.enum(["NoNumbers", "Verified", "Unverified"]),
      }),
    })
    .transform((v) =>
      chunk({
        kind: "number-check",
        check: numberChecks[v.NumberCheck.status],
      }),
    ),
  z
    .strictObject({ Done: z.looseObject({}) })
    .transform(() => end({ kind: "done" })),
  z
    .strictObject({ RateLimited: z.looseObject({}) })
    .transform(() => end({ kind: "rate-limited" })),
  z
    .strictObject({ Failed: z.object({ reason: failureSchema }) })
    .transform((v) => end({ kind: "failed", reason: v.Failed.reason })),
  z.literal("Cancelled").transform(() => end({ kind: "cancelled" })),
  z.literal("Interrupted").transform(() => end({ kind: "interrupted" })),
]);
const eventSchema = z
  .strictObject({
    sequence: sequenceSchema,
    turn_id: idSchema,
    chunk: chunkSchema,
  })
  .transform((value): ChatStreamEvent => ({
    sequence: value.sequence,
    turn: value.turn_id,
    chunk: value.chunk,
  }));

const entrySchema = z
  .object({
    turn_id: idSchema,
    role: z
      .enum(["User", "Assistant"])
      .transform((key): ChatRole => (key === "User" ? "user" : "assistant")),
    text: bodySchema,
    status: turnStatusSchema.nullish(),
    cards: z.array(cardSchema),
    sources: z.array(sourceSchema),
  })
  .transform((value): ChatHistoryEntry => ({
    turn: value.turn_id,
    role: value.role,
    text: value.text,
    status: value.status ?? undefined,
    cards: value.cards,
    sources: value.sources,
  }));

const historySchema = z
  .object({
    entries: z.array(entrySchema),
    active_turn: z.object({ turn_id: idSchema, state: activeSchema }).nullish(),
  })
  .transform((value): ChatHistory => ({
    entries: value.entries,
    active:
      value.active_turn === null || value.active_turn === undefined
        ? undefined
        : { turn: value.active_turn.turn_id, state: value.active_turn.state },
  }));

const turnSchema = z
  .object({ turn_id: idSchema, status: turnStatusSchema })
  .transform((value): ChatTurnSettled => ({ status: value.status }));

const cancelSchema = z
  .object({ outcome: z.enum(["Signaled", "NotRunning"]) })
  .transform((value): ChatCancelOutcome => ({
    outcome: value.outcome === "Signaled" ? "signaled" : "not-running",
  }));

const resubscribeSchema = z
  .object({ state: activeSchema })
  .transform((value): ChatResubscribed => ({ state: value.state }));

export const parseChatStreamEvent = (
  raw: Wire,
): Result<ChatStreamEvent, IpcFailure> => {
  const parsed = eventSchema.safeParse(raw);
  return parsed.success ? ok(parsed.data) : err({ kind: "invalid-response" });
};

export const parseChatId = (raw: Wire): Result<EntityId, IpcFailure> => {
  const parsed = idSchema.safeParse(raw);
  return parsed.success ? ok(parsed.data) : err({ kind: "invalid-response" });
};

export const settleChatHistory = (
  outcome: WireOutcome,
): Result<ChatHistory, IpcFailure> => settleOutcome(outcome, historySchema);

export const settleChatTurn = (
  outcome: WireOutcome,
): Result<ChatTurnSettled, IpcFailure> => settleOutcome(outcome, turnSchema);

export const settleChatCancel = (
  outcome: WireOutcome,
): Result<ChatCancelOutcome, IpcFailure> =>
  settleOutcome(outcome, cancelSchema);

export const settleChatResubscribe = (
  outcome: WireOutcome,
): Result<ChatResubscribed, IpcFailure> =>
  settleOutcome(outcome, resubscribeSchema);
