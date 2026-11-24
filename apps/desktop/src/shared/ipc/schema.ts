import { z } from "zod";
import { err, ok, type Result } from "../lib";

type Brand<N extends string> = Readonly<z.core.$brand<N>>;
type AppVersionText = string & Brand<"AppVersionText">;
export type AppVersion = { readonly version: AppVersionText };
type CommandErrorKind = "user-fixable" | "retryable" | "bug" | "no-data";

export type AppVersionText = string & Brand<"AppVersionText">;

export type AppVersion = { readonly version: AppVersionText };

export type CommandErrorKind = "user-fixable" | "retryable" | "bug" | "no-data";

export type CommandError = { readonly kind: CommandErrorKind };
export type IpcFailure =
  | { readonly kind: "command"; readonly error: CommandError }
  | { readonly kind: "framework" }
  | { readonly kind: "invalid-response" }
  | { readonly kind: "timeout" };

type AllEventKind = "StoredDataChanged" | "AccountSelectionChanged";
type SettingsEventKind =
  "SettingsChanged" | "PolicyChanged" | "OrderSwitchChanged";
type PortfolioEventKind =
  | "PortfolioSyncRecorded"
  | "ValuationSnapshotTaken"
  | "PriceUpdated"
  | "PriceMoved"
  | "FxRateUpdated"
  | "WatchlistChanged"
  | "MarketSessionChanged";
type TransactionEventKind =
  | "TransactionsSyncRecorded"
  | "OrderFilled"
  | "OrderCancelled"
  | "OrderModified";
type StatusEventKind =
  | "SelfCheckChanged"
  | "BrokerConnectionChanged"
  | "AiBudgetChanged"
  | "CollectionFinished";
type ActivityEventKind =
  | "DisclosureArrived"
  | "DisclosureTriggerRecorded"
  | "NewsActivityIncreased"
  | "NewsClusterGrew"
  | "ProposalCreated"
  | "ProposalExpired"
  | "ProposalApproved"
  | "ProposalRejected"
  | "ProposalSkipped"
  | "OrderSubmitted"
  | "OrderRejected"
  | "OrderOutcomeUnknown"
  | "OrderOutcomeResolved"
  | "ThesisUpdated"
  | "SignalRaised"
  | "SignalCleared"
  | "EstimateSubmitted"
  | "EstimateResolved";

export type CoreUiEvent =
  | { readonly category: "all"; readonly kind: AllEventKind }
  | { readonly category: "settings"; readonly kind: SettingsEventKind }
  | { readonly category: "portfolio"; readonly kind: PortfolioEventKind }
  | { readonly category: "transactions"; readonly kind: TransactionEventKind }
  | { readonly category: "status"; readonly kind: StatusEventKind }
  | { readonly category: "activity"; readonly kind: ActivityEventKind };

type CommandOutcome =
  | { readonly status: "ok"; readonly data: unknown }
  | { readonly status: "error"; readonly error: unknown };

const frameworkFailure: IpcFailure = { kind: "framework" };
const invalidResponseFailure: IpcFailure = { kind: "invalid-response" };

const appVersionSchema = z
  .object({
    version: z
      .string()
      .regex(/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/u)
      .brand<"AppVersionText">(),
  })
  .strict()
  .readonly();

const commandErrorSchema = z
  .object({ kind: z.enum(["user-fixable", "retryable", "bug", "no-data"]) })
  .strict()
  .readonly();

const nonEmptyId = z.string().min(1);
const marketSchema = z.enum(["Kr", "Us"]);
const fxPairSchema = z.enum(["UsdKrw", "KrwUsd"]);
const disclosureSourceSchema = z.enum(["Kind", "Dart", "SecEdgar"]);
const newsSourceSchema = z.enum([
  "Hankyung",
  "Maekyung",
  "Yonhap",
  "Edaily",
  "BankOfKorea",
  "FederalReserve",
  "GoogleNews",
  "YahooFinance",
]);
const disclosureKeySchema = z
  .object({ source: disclosureSourceSchema, receipt_no: nonEmptyId })
  .strict();
const informationSourceSchema = z.union([
  z.literal("Web"),
  z.object({ News: newsSourceSchema }).strict(),
  z.object({ Disclosure: disclosureSourceSchema }).strict(),
]);

const unitEntry = (event: CoreUiEvent): readonly [string, CoreUiEvent] => [
  event.kind,
  event,
];

const unitEvents: ReadonlyMap<string, CoreUiEvent> = new Map<
  string,
  CoreUiEvent
>([
  unitEntry({ category: "all", kind: "StoredDataChanged" }),
  unitEntry({ category: "all", kind: "AccountSelectionChanged" }),
  unitEntry({ category: "settings", kind: "SettingsChanged" }),
  unitEntry({ category: "settings", kind: "PolicyChanged" }),
  unitEntry({ category: "settings", kind: "OrderSwitchChanged" }),
  unitEntry({ category: "portfolio", kind: "PortfolioSyncRecorded" }),
  unitEntry({ category: "portfolio", kind: "ValuationSnapshotTaken" }),
  unitEntry({ category: "portfolio", kind: "WatchlistChanged" }),
  unitEntry({ category: "transactions", kind: "TransactionsSyncRecorded" }),
  unitEntry({ category: "transactions", kind: "OrderFilled" }),
  unitEntry({ category: "transactions", kind: "OrderCancelled" }),
  unitEntry({ category: "transactions", kind: "OrderModified" }),
  unitEntry({ category: "status", kind: "SelfCheckChanged" }),
  unitEntry({ category: "status", kind: "BrokerConnectionChanged" }),
  unitEntry({ category: "status", kind: "AiBudgetChanged" }),
]);

type KeyedEntry = {
  readonly event: CoreUiEvent;
  readonly schema: Readonly<z.ZodType>;
};

const keyedEntry = (
  event: CoreUiEvent,
  schema: Readonly<z.ZodType>,
): readonly [string, KeyedEntry] => [event.kind, { event, schema }];

const idActivityKinds: readonly ActivityEventKind[] = [
  "NewsActivityIncreased",
  "NewsClusterGrew",
  "ProposalCreated",
  "ProposalExpired",
  "ProposalApproved",
  "ProposalRejected",
  "ProposalSkipped",
  "OrderSubmitted",
  "OrderRejected",
  "OrderOutcomeUnknown",
  "OrderOutcomeResolved",
  "ThesisUpdated",
  "SignalRaised",
  "SignalCleared",
  "EstimateSubmitted",
  "EstimateResolved",
];

const keyedEvents: ReadonlyMap<string, KeyedEntry> = new Map<
  string,
  KeyedEntry
>([
  keyedEntry({ category: "portfolio", kind: "PriceUpdated" }, nonEmptyId),
  keyedEntry({ category: "portfolio", kind: "PriceMoved" }, nonEmptyId),
  keyedEntry({ category: "portfolio", kind: "FxRateUpdated" }, fxPairSchema),
  keyedEntry(
    { category: "portfolio", kind: "MarketSessionChanged" },
    marketSchema,
  ),
  keyedEntry(
    { category: "status", kind: "CollectionFinished" },
    informationSourceSchema,
  ),
  keyedEntry(
    { category: "activity", kind: "DisclosureArrived" },
    disclosureKeySchema,
  ),
  keyedEntry(
    { category: "activity", kind: "DisclosureTriggerRecorded" },
    disclosureKeySchema,
  ),
  ...idActivityKinds.map((kind) =>
    keyedEntry({ category: "activity", kind }, nonEmptyId),
  ),
]);

const singleKeyRecordSchema = z.record(z.string(), z.unknown());

const parseKeyedUiEvent = (
  payload: unknown,
): Result<CoreUiEvent, IpcFailure> => {
  const record = singleKeyRecordSchema.safeParse(payload);
  if (!record.success) return err(invalidResponseFailure);
  const entries = Object.entries(record.data);
  if (entries.length !== 1) return err(invalidResponseFailure);
  const first = entries[0];
  const entry = first === undefined ? undefined : keyedEvents.get(first[0]);
  if (first === undefined || entry === undefined)
    return err(invalidResponseFailure);
  return entry.schema.safeParse(first[1]).success
    ? ok(entry.event)
    : err(invalidResponseFailure);
};

export const parseUiEvent = (
  payload: unknown,
): Result<CoreUiEvent, IpcFailure> => {
  if (typeof payload !== "string") return parseKeyedUiEvent(payload);
  const matched = unitEvents.get(payload);
  return matched === undefined ? err(invalidResponseFailure) : ok(matched);
};
type OpenExternalStatus = "opened";

export type OpenExternalResult = { readonly status: OpenExternalStatus };

const openExternalResultSchema = z
  .object({ status: z.literal("opened") })
  .strict()
  .readonly();

const settleWithSchema = <T>(
  outcome: CommandOutcome,
  schema: Readonly<z.ZodType<T>>,
): Result<T, IpcFailure> => {
  switch (outcome.status) {
    case "ok": {
      const parsed = schema.safeParse(outcome.data);
      return parsed.success ? ok(parsed.data) : err(invalidResponseFailure);
    }
    case "error": {
      const parsed = commandErrorSchema.safeParse(outcome.error);
      return parsed.success
        ? err({ kind: "command", error: parsed.data })
        : err(frameworkFailure);
    }
  }
};

export const settleAppVersion = (
  outcome: CommandOutcome,
): Result<AppVersion, IpcFailure> =>
  settleWithSchema(outcome, appVersionSchema);

export const settleOpenExternal = (
  outcome: CommandOutcome,
): Result<OpenExternalResult, IpcFailure> =>
  settleWithSchema(outcome, openExternalResultSchema);
