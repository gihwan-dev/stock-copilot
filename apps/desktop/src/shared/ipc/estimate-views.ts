import type {
  ChartDecimal,
  EntityId,
  ExternalUrl,
  FreeText,
  InputValue,
  Money,
  PercentDisplay,
  Quantity,
  Result,
  Ticker,
  Timestamp,
} from "../lib";
import type { IpcFailure } from "./schema";

export type SignalSourceKind = "tool" | "web" | "news" | "disclosure";

export type SignalSource = {
  readonly id: EntityId;
  readonly kind: SignalSourceKind;
  readonly label: FreeText;
};

export type SignalSizing =
  | {
      readonly kind: "sized";
      readonly quantity: Quantity;
      readonly amount: Money;
      readonly costs: Money;
      readonly weightAfter: PercentDisplay;
    }
  | { readonly kind: "zero"; readonly reason: FreeText }
  | { readonly kind: "pending" };

export type SignalCardView = {
  readonly id: EntityId;
  readonly name: FreeText;
  readonly ticker: Ticker;
  readonly direction: "buy" | "sell";
  readonly market: "open" | "closed";
  readonly orderSwitch: "on" | "off";
  readonly proposal: EntityId | undefined;
  readonly price: Money | undefined;
  readonly sizing: SignalSizing;
  readonly aiProbability: PercentDisplay;
  readonly trackedProbability: PercentDisplay;
  readonly threshold: PercentDisplay;
  readonly binHitRate: PercentDisplay | undefined;
  readonly binCount: ChartDecimal;
  readonly rationale: FreeText;
  readonly counterargument: FreeText;
  readonly sources: readonly SignalSource[];
};

export type SignalsView = { readonly cards: readonly SignalCardView[] };

export type ScorecardBinRow = {
  readonly id: EntityId;
  readonly label: FreeText;
  readonly meanPrediction: ChartDecimal;
  readonly hitRate: ChartDecimal;
  readonly count: ChartDecimal;
};

export type ScorecardCombination = {
  readonly id: EntityId;
  readonly model: FreeText;
  readonly horizon: FreeText;
  readonly judged: ChartDecimal;
  readonly verdict: "pending" | "ready";
  readonly brier: ChartDecimal | undefined;
  readonly bins: readonly ScorecardBinRow[];
  readonly signalReturn: PercentDisplay | undefined;
  readonly benchmarkReturn: PercentDisplay | undefined;
  readonly intervalLow: PercentDisplay | undefined;
  readonly intervalHigh: PercentDisplay | undefined;
  readonly comparisons: ChartDecimal;
};

export type ScorecardScreenView = {
  readonly combinations: readonly ScorecardCombination[];
};

export type InstrumentListRow = {
  readonly id: EntityId;
  readonly name: FreeText;
  readonly ticker: Ticker;
  readonly origin: "holding" | "watchlist";
  readonly todayRate: PercentDisplay | undefined;
  readonly direction: "up" | "down" | "flat";
};

export type InstrumentListView = {
  readonly rows: readonly InstrumentListRow[];
};

export type InstrumentCandle = {
  readonly id: EntityId;
  readonly label: FreeText;
  readonly open: ChartDecimal;
  readonly high: ChartDecimal;
  readonly low: ChartDecimal;
  readonly close: ChartDecimal;
};

export type InstrumentNewsLine = {
  readonly id: EntityId;
  readonly title: FreeText;
  readonly source: FreeText;
  readonly publishedAt: Timestamp;
  readonly url: ExternalUrl | undefined;
};

export type InstrumentDetailView = {
  readonly id: EntityId;
  readonly name: FreeText;
  readonly ticker: Ticker;
  readonly price: Money;
  readonly todayRate: PercentDisplay | undefined;
  readonly direction: "up" | "down" | "flat";
  readonly quotedAt: Timestamp;
  readonly candles: readonly InstrumentCandle[];
  readonly holding:
    | {
        readonly quantity: Quantity;
        readonly value: Money;
        readonly profit: Money;
        readonly profitRate: PercentDisplay;
      }
    | undefined;
  readonly news?: readonly InstrumentNewsLine[];
};

export type BriefingKind = "morning" | "closing-review" | "pre-us-market";

export type BriefingSourceKind = "web" | "news" | "disclosure";

export type BriefingFailure =
  "ai-unavailable" | "rate-limited" | "empty" | "rejected";

export type BriefingSourceView = {
  readonly id: EntityId;
  readonly title: FreeText;
  readonly url: ExternalUrl | undefined;
  readonly kind: BriefingSourceKind;
};

export type BriefingOutcome =
  | {
      readonly kind: "completed";
      readonly body: FreeText;
      readonly sources: readonly BriefingSourceView[];
    }
  | { readonly kind: "failed"; readonly reason: BriefingFailure };

export type BriefingSummary = {
  readonly id: EntityId;
  readonly kind: BriefingKind;
  readonly finishedAt: Timestamp;
  readonly outcome: BriefingOutcome;
};

export type BriefingsView = {
  readonly latest: BriefingSummary | undefined;
  readonly past: readonly BriefingSummary[];
};

export type ThesisClaim = {
  readonly id: EntityId;
  readonly text: FreeText;
  readonly status: "confirmed" | "needs-review";
};

export type ThesisRevision = {
  readonly id: EntityId;
  readonly at: Timestamp | undefined;
  readonly summary: FreeText;
};

export type ThesisView = {
  readonly claims: readonly ThesisClaim[];
  readonly history: readonly ThesisRevision[];
};

export type OrderRequestOutcome =
  | "requested"
  | "outcome-unknown"
  | "presence-denied"
  | "switch-off"
  | "hourly-limit"
  | "market-closed"
  | "dialog-in-progress"
  | "cooldown"
  | "already-consumed"
  | "proposal-unavailable"
  | "conditions-changed"
  | "high-value-rejected"
  | "broker-rejected"
  | "unavailable"
  | "other";

type Load<T> = () => Promise<Result<T, IpcFailure>>;
type LoadFor<T> = (id: EntityId) => Promise<Result<T, IpcFailure>>;

export type WatchlistChange =
  | { readonly kind: "applied" }
  | { readonly kind: "rejected"; readonly reason: FreeText };

export type EstimateSources = {
  readonly signals: Load<SignalsView>;
  readonly scorecard: Load<ScorecardScreenView>;
  readonly instruments: Load<InstrumentListView>;
  readonly instrumentDetail: LoadFor<InstrumentDetailView>;
  readonly briefings: Load<BriefingsView>;
  readonly thesis: LoadFor<ThesisView>;
  readonly requestOrderApproval: (
    proposal: EntityId,
  ) => Promise<Result<OrderRequestOutcome, IpcFailure>>;
  readonly addWatchlist: (
    ticker: Ticker,
  ) => Promise<Result<WatchlistChange, IpcFailure>>;
  readonly removeWatchlist: (
    instrument: EntityId,
  ) => Promise<Result<WatchlistChange, IpcFailure>>;
  readonly skipSignal: (
    card: EntityId,
    reason: InputValue,
  ) => Promise<Result<"recorded", IpcFailure>>;
};
