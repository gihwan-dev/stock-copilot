import type {
  ChatCardKind,
  ChatFailureReason,
  ChatSourceKind,
  ChatToolTopic,
} from "../../../shared/ipc";
import type { UiText } from "../../../shared/lib";
import { chatText, type ChatMessageKey } from "../messages";

const toolKeys = {
  portfolio: "chat.tool.portfolio",
  allocation: "chat.tool.allocation",
  transactions: "chat.tool.transactions",
  performance: "chat.tool.performance",
  quote: "chat.tool.quote",
  candles: "chat.tool.candles",
  market: "chat.tool.market",
  rankings: "chat.tool.rankings",
  "investor-flow": "chat.tool.investorFlow",
  news: "chat.tool.news",
  disclosures: "chat.tool.disclosures",
  article: "chat.tool.article",
  thesis: "chat.tool.thesis",
  profile: "chat.tool.profile",
  estimate: "chat.tool.estimate",
  scorecard: "chat.tool.scorecard",
  "web-research": "chat.tool.webResearch",
  other: "chat.tool.other",
} as const satisfies Record<ChatToolTopic, ChatMessageKey>;

const failureKeys = {
  isolation: "chat.failedSafety",
  "forbidden-tool": "chat.failedSafety",
  connection: "chat.failedConnection",
  credential: "chat.failedCredential",
  "invalid-response": "chat.failedOther",
  storage: "chat.failedOther",
  "empty-message": "chat.failedMessage",
  "too-long": "chat.failedMessage",
  busy: "chat.failedBusy",
} as const satisfies Record<ChatFailureReason, ChatMessageKey>;

const sourceKeys = {
  tool: "chat.source.tool",
  disclosure: "chat.source.disclosure",
  news: "chat.source.news",
  web: "chat.source.web",
} as const satisfies Record<ChatSourceKind, ChatMessageKey>;

const cardKeys = {
  signal: "chat.card.signal",
  proposal: "chat.card.proposal",
  holding: "chat.card.holding",
  quote: "chat.card.quote",
  briefing: "chat.card.briefing",
  disclosure: "chat.card.disclosure",
} as const satisfies Record<ChatCardKind, ChatMessageKey>;

export const toolLabel = (topic: ChatToolTopic): UiText =>
  chatText(toolKeys[topic], "ko");

export const failureLabel = (reason: ChatFailureReason | undefined): UiText =>
  chatText(
    reason === undefined ? "chat.failedOther" : failureKeys[reason],
    "ko",
  );

export const sourceLabel = (kind: ChatSourceKind): UiText =>
  chatText(sourceKeys[kind], "ko");

export const cardLabel = (kind: ChatCardKind): UiText =>
  chatText(cardKeys[kind], "ko");
