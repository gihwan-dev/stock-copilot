import type { Count, EntityId, ExternalUrl, FreeText, Ticker } from "../lib";

export type ChatSessionItem = {
  readonly session: EntityId;
  readonly title: FreeText | undefined;
};

export type ChatChangeTopic =
  | "change-schedule"
  | "change-profile"
  | "change-order-settings"
  | "add-watchlist"
  | "remove-watchlist"
  | "skip-signal"
  | "change-account"
  | "change-principles"
  | "backup"
  | "export"
  | "change-news"
  | "change-contact"
  | "change-wake"
  | "change-aliases"
  | "change-model"
  | "change-budget"
  | "approval-request";

export type ChatToolTopic =
  | "portfolio"
  | "allocation"
  | "transactions"
  | "performance"
  | "quote"
  | "candles"
  | "market"
  | "rankings"
  | "investor-flow"
  | "news"
  | "disclosures"
  | "article"
  | "thesis"
  | "profile"
  | "estimate"
  | "scorecard"
  | "web-research"
  | "app-data"
  | ChatChangeTopic
  | "other";

export const chatChangeTopics: readonly ChatChangeTopic[] = [
  "change-schedule",
  "change-profile",
  "change-order-settings",
  "add-watchlist",
  "remove-watchlist",
  "skip-signal",
  "change-account",
  "change-principles",
  "backup",
  "export",
  "change-news",
  "change-contact",
  "change-wake",
  "change-aliases",
  "change-model",
  "change-budget",
  "approval-request",
];

export type ChatToolStatus = "running" | "succeeded" | "failed";

export type ChatCardKind =
  "signal" | "proposal" | "holding" | "quote" | "briefing" | "disclosure";

export type ChatCardRef = {
  readonly kind: ChatCardKind;
  readonly id: EntityId;
};

export type ChatNumberCheck =
  "no-numbers" | "verified" | "computed" | "unverified";

export type ChatSourceKind = "tool" | "disclosure" | "news" | "web";

export type ChatSource = {
  readonly kind: ChatSourceKind;
  readonly reference: FreeText;
  readonly url: ExternalUrl | undefined;
};

export type ChatFailureReason =
  | "isolation"
  | "connection"
  | "credential"
  | "invalid-response"
  | "forbidden-tool"
  | "empty-message"
  | "too-long"
  | "storage"
  | "busy";

export type ChatContentChunk =
  | { readonly kind: "text"; readonly text: FreeText }
  | { readonly kind: "tool-started"; readonly topic: ChatToolTopic }
  | {
      readonly kind: "tool-finished";
      readonly topic: ChatToolTopic;
      readonly status: ChatToolStatus;
    }
  | { readonly kind: "card"; readonly card: ChatCardRef }
  | { readonly kind: "sources"; readonly items: readonly ChatSource[] }
  | { readonly kind: "number-check"; readonly check: ChatNumberCheck };

export type ChatEndChunk =
  | {
      readonly kind: "done" | "cancelled" | "interrupted" | "rate-limited";
    }
  | { readonly kind: "failed"; readonly reason: ChatFailureReason };

export type ChatStreamChunk =
  | { readonly kind: "content"; readonly content: ChatContentChunk }
  | { readonly kind: "end"; readonly end: ChatEndChunk };

export type ChatStreamEvent = {
  readonly sequence: Count;
  readonly turn: EntityId;
  readonly chunk: ChatStreamChunk;
};

export type ChatTurnStatus =
  | "in-progress"
  | "completed"
  | "cancelled"
  | "interrupted"
  | "rate-limited"
  | "failed";

export type ChatRole = "user" | "assistant";

export type ChatHistoryEntry = {
  readonly turn: EntityId;
  readonly role: ChatRole;
  readonly text: FreeText;
  readonly status: ChatTurnStatus | undefined;
  readonly cards: readonly ChatCardRef[];
  readonly sources: readonly ChatSource[];
};

export type ChatActiveState = "running" | "finished";

export type ChatActiveTurn = {
  readonly turn: EntityId;
  readonly state: ChatActiveState;
};

export type ChatHistory = {
  readonly entries: readonly ChatHistoryEntry[];
  readonly active: ChatActiveTurn | undefined;
};

export type ChatTurnSettled = { readonly status: ChatTurnStatus };

export type ChatCancelOutcome = {
  readonly outcome: "signaled" | "not-running";
};

export type ChatResubscribed = { readonly state: ChatActiveState };

export type ChatHorizon =
  "few-days" | "weeks-to-months" | "half-year" | "hours";

export type ChatMarket = "kr" | "us";

export type ChatScreenContext = {
  readonly symbol: Ticker | undefined;
  readonly market: ChatMarket | undefined;
  readonly horizon: ChatHorizon | undefined;
};

export type ChatTurnStart = {
  readonly session: EntityId;
  readonly turn: EntityId;
  readonly message: FreeText;
  readonly context: ChatScreenContext | undefined;
};
