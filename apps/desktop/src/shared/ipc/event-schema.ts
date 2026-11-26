import { z } from "zod";
import { err, ok, type Result } from "../lib";
import type { IpcFailure } from "./schema";

const invalidResponseFailure: IpcFailure = { kind: "invalid-response" };

type AllEventKind =
  | "StoredDataChanged"
  | "AccountSelectionChanged"
  | "AiCommandApplied"
  | "AiApprovalRequested";
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
  | { readonly category: "activity"; readonly kind: ActivityEventKind }
  | { readonly category: "briefing"; readonly kind: "BriefingFinished" };

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

const briefingIdSchema = z
  .object({
    kind: z.enum(["Morning", "ClosingReview", "PreUsMarket"]),
    date: nonEmptyId,
  })
  .strict();

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
  keyedEntry(
    { category: "briefing", kind: "BriefingFinished" },
    briefingIdSchema,
  ),
  keyedEntry({ category: "all", kind: "AiCommandApplied" }, nonEmptyId),
  keyedEntry({ category: "all", kind: "AiApprovalRequested" }, nonEmptyId),
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
