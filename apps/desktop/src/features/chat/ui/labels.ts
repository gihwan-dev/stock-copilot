import type {
  ChatCardKind,
  ChatChangeTopic,
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
  "app-data": "chat.tool.appData",
  "change-schedule": "chat.tool.appChange",
  "change-profile": "chat.tool.appChange",
  "change-order-settings": "chat.tool.appChange",
  "add-watchlist": "chat.tool.appChange",
  "remove-watchlist": "chat.tool.appChange",
  "skip-signal": "chat.tool.appChange",
  "change-account": "chat.tool.appChange",
  "change-principles": "chat.tool.appChange",
  "backup": "chat.tool.appChange",
  "export": "chat.tool.appChange",
  "change-news": "chat.tool.appChange",
  "change-contact": "chat.tool.appChange",
  "change-wake": "chat.tool.appChange",
  "change-aliases": "chat.tool.appChange",
  "change-model": "chat.tool.appChange",
  "change-budget": "chat.tool.appChange",
  "approval-request": "chat.tool.approval",
  other: "chat.tool.other",
} as const satisfies Record<ChatToolTopic, ChatMessageKey>;

const changeKeys = {
  "change-schedule": "chat.changed.changeSchedule",
  "change-profile": "chat.changed.changeProfile",
  "change-order-settings": "chat.changed.changeOrderSettings",
  "add-watchlist": "chat.changed.addWatchlist",
  "remove-watchlist": "chat.changed.removeWatchlist",
  "skip-signal": "chat.changed.skipSignal",
  "change-account": "chat.changed.changeAccount",
  "change-principles": "chat.changed.changePrinciples",
  "backup": "chat.changed.backup",
  "export": "chat.changed.export",
  "change-news": "chat.changed.changeNews",
  "change-contact": "chat.changed.changeContact",
  "change-wake": "chat.changed.changeWake",
  "change-aliases": "chat.changed.changeAliases",
  "change-model": "chat.changed.changeModel",
  "change-budget": "chat.changed.changeBudget",
  "approval-request": "chat.changed.approvalRequest",
} as const satisfies Record<ChatChangeTopic, ChatMessageKey>;

export const changeLabel = (topic: ChatChangeTopic): UiText =>
  chatText(changeKeys[topic], "ko");

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
