import { z } from "zod";
import {
  chatIdSchema,
  freeTextSchema,
  err,
  type EntityId,
  type Result,
} from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import type { ChatSessionItem } from "./chat-types";
import {
  settleWithSchema,
  wireOutcome,
  type IpcFailure,
  type WireOutcome,
} from "./schema";

const sessionItemSchema = z
  .object({ session_id: chatIdSchema, title: freeTextSchema.nullable() })
  .transform((item): ChatSessionItem => ({
    session: item.session_id,
    title: item.title ?? undefined,
  }));

const settleChatSessions = (
  outcome: WireOutcome,
): Result<readonly ChatSessionItem[], IpcFailure> =>
  settleWithSchema(outcome, z.array(sessionItemSchema));
const settleCreatedChatSession = (
  outcome: WireOutcome,
): Result<ChatSessionItem, IpcFailure> =>
  settleWithSchema(outcome, sessionItemSchema);

export const chatSessions = (): Promise<
  Result<readonly ChatSessionItem[], IpcFailure>
> =>
  settleCall(commands.listChatSessions(), (outcome) =>
    settleChatSessions(wireOutcome(outcome)),
  );

export const createChatSession = (
  session: EntityId,
): Promise<Result<ChatSessionItem, IpcFailure>> =>
  settleCall(commands.createChatSession({ session_id: session }), (outcome) => {
    const result = settleCreatedChatSession(wireOutcome(outcome));
    return result.kind === "ok" && result.value.session !== session
      ? err({ kind: "invalid-response" })
      : result;
  });
