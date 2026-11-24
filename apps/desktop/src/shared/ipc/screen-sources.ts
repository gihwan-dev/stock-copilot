import {
  err,
  type ChartDecimal,
  type Currency,
  type EntityId,
  type FreeText,
  type Money,
  type PercentDisplay,
  type Quantity,
  type Result,
  type Ticker,
  type Timestamp,
} from "../lib";
import type {
  BriefingsView,
  EstimateSources,
  InstrumentDetailView,
  InstrumentListView,
  ScorecardScreenView,
  SignalsView,
  ThesisView,
  WatchlistChange,
} from "./estimate-views";
import type {
  OrderSettingsDraft,
  ProfileDraft,
  RiskEditorSources,
  RiskSaveOutcome,
} from "./risk-editor";
import type { IpcFailure } from "./schema";

type PriceDirection = "up" | "down" | "flat";

type SignedAmount = {
  readonly amount: Money;
  readonly rate: PercentDisplay;
  readonly direction: PriceDirection;
};

export type HoldingRow = {
  readonly id: EntityId;
  readonly name: FreeText;
  readonly ticker: Ticker;
  readonly quantity: Quantity;
  readonly value: Money;
  readonly profit: SignedAmount;
};

export type CurrencyValue = {
  readonly currency: Currency;
  readonly holdings: Money;
  readonly buyingPower: Money;
};

type ExchangeRateNote = {
  readonly kind: "mid-rate";
  readonly at: Timestamp;
};

type DataConnection = "live" | "stored";

export type PortfolioView = {
  readonly recordedAt: Timestamp;
  readonly connection: DataConnection;
  readonly totalApproximate: Money;
  readonly exchangeRate: ExchangeRateNote | undefined;
  readonly currencies: readonly CurrencyValue[];
  readonly holdings: readonly HoldingRow[];
  readonly today: SignedAmount;
};

export type AllocationRow = {
  readonly id: EntityId;
  readonly label: FreeText;
  readonly value: Money;
  readonly weight: PercentDisplay;
  readonly level: "normal" | "warning" | "over-cap";
};

type ConcentrationState = "balanced" | "concentrated";

export type AllocationView = {
  readonly instruments: readonly AllocationRow[];
  readonly sectors: readonly AllocationRow[];
  readonly concentration: ConcentrationState;
};

export type TransactionKind =
  "buy" | "sell" | "dividend" | "deposit" | "withdrawal" | "fee" | "other";

type TransactionPresence = "present" | "vanished";

export type TransactionRow = {
  readonly id: EntityId;
  readonly settledAt: Timestamp;
  readonly name: FreeText;
  readonly kind: TransactionKind;
  readonly quantity: Quantity | undefined;
  readonly amount: Money;
  readonly presence: TransactionPresence;
};

export type TransactionsView = {
  readonly recordedAt: Timestamp;
  readonly rows: readonly TransactionRow[];
};

export type SelfCheckProblem =
  | "toss-key-invalid"
  | "toss-key-permission"
  | "toss-account-missing"
  | "ip-not-allowed"
  | "toss-key-shared"
  | "claude-missing"
  | "claude-signed-out"
  | "order-risk-notice"
  | "order-exchange-setting"
  | "order-dollar-shortage";

export type SelfCheckView =
  | { readonly kind: "clear" }
  | { readonly kind: "problem"; readonly problem: SelfCheckProblem };

export type CheckRecord = {
  readonly id: EntityId;
  readonly at: Timestamp;
  readonly check: FreeText;
  readonly detail: FreeText;
};

type AiRuntimeState =
  | { readonly kind: "ready" }
  | { readonly kind: "unavailable"; readonly reason: FreeText };

type AiLimitState =
  | { readonly kind: "available" }
  | { readonly kind: "waiting"; readonly resumesAt: Timestamp };

export type SourceStatus = {
  readonly id: EntityId;
  readonly name: FreeText;
  readonly lastSuccess: Timestamp | undefined;
};

export type AiUsageRow = {
  readonly id: EntityId;
  readonly task: FreeText;
  readonly tokens: ChartDecimal;
  readonly seconds: ChartDecimal;
};

export type StatusView = {
  readonly records: readonly CheckRecord[];
  readonly aiRuntime: AiRuntimeState;
  readonly aiLimit: AiLimitState;
  readonly sources: readonly SourceStatus[];
  readonly aiUsage: readonly AiUsageRow[];
};

type Load<T> = () => Promise<Result<T, IpcFailure>>;

export type ScreenSources = EstimateSources &
  RiskEditorSources & {
    readonly portfolio: Load<PortfolioView>;
    readonly allocation: Load<AllocationView>;
    readonly transactions: Load<TransactionsView>;
    readonly selfCheck: Load<SelfCheckView>;
    readonly status: Load<StatusView>;
  };

const pending = <T>(): Load<T> => {
  const noData: IpcFailure = { kind: "command", error: { kind: "no-data" } };
  return (): Promise<Result<T, IpcFailure>> => Promise.resolve(err(noData));
};

export const pendingScreenSources: ScreenSources = {
  signals: pending<SignalsView>(),
  scorecard: pending<ScorecardScreenView>(),
  instruments: pending<InstrumentListView>(),
  instrumentDetail: pending<InstrumentDetailView>(),
  briefings: pending<BriefingsView>(),
  thesis: pending<ThesisView>(),
  requestOrderApproval: pending<"requested">(),
  skipSignal: pending<"recorded">(),
  addWatchlist: pending<WatchlistChange>(),
  removeWatchlist: pending<WatchlistChange>(),
  investorProfile: pending<ProfileDraft>(),
  orderSettings: pending<OrderSettingsDraft>(),
  saveInvestorProfile: pending<RiskSaveOutcome>(),
  saveOrderSettings: pending<RiskSaveOutcome>(),
  portfolio: pending<PortfolioView>(),
  allocation: pending<AllocationView>(),
  transactions: pending<TransactionsView>(),
  selfCheck: pending<SelfCheckView>(),
  status: pending<StatusView>(),
};
