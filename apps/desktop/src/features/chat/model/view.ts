import type { EntityId, ExternalUrl } from "../../../shared/lib";
import type { AssistantReply, Transcript } from "./transcript";

export type PanelState =
  "empty" | "looking-up" | "streaming" | "failed" | "idle";

const settledStates = {
  done: "idle",
  cancelled: "idle",
  failed: "failed",
  interrupted: "failed",
  "rate-limited": "failed",
} as const;

export const panelState = (transcript: Transcript): PanelState => {
  const last = transcript.exchanges.at(-1);
  if (last === undefined) return "empty";
  if (last.reply.phase !== "streaming") return settledStates[last.reply.phase];
  return last.reply.tool === undefined ? "streaming" : "looking-up";
};

export const streamingTurn = (transcript: Transcript): EntityId | undefined => {
  const last = transcript.exchanges.at(-1);
  return last?.reply.phase === "streaming" ? last.turn : undefined;
};

export const linkVerdict = (
  reply: AssistantReply,
  url: ExternalUrl,
): "allow" | "deny" =>
  reply.sources.some((source) => source.url === url) ? "allow" : "deny";
