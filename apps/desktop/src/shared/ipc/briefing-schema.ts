import { z } from "zod";
import {
  chatIdSchema,
  externalUrlSchema,
  freeTextSchema,
  parseEntityId,
  parseTimestamp,
  text,
  type EntityId,
  type Result,
} from "../lib";
import type {
  BriefingFailure,
  BriefingKind,
  BriefingSourceView,
  BriefingSummary,
  BriefingsView,
} from "./estimate-views";
import {
  settleWithSchema,
  wire,
  type IpcFailure,
  type Wire,
  type WireOutcome,
} from "./schema";
import { uiFreeText } from "./wire-values";

const kindCodes = {
  Morning: "morning",
  ClosingReview: "closing-review",
  PreUsMarket: "pre-us-market",
} as const;

const failureCodes = {
  "ai-unavailable": "ai-unavailable",
  "rate-limited": "rate-limited",
  empty: "empty",
  rejected: "rejected",
} as const;

const linkSchema = z.union([
  externalUrlSchema,
  z.unknown().transform((): undefined => undefined),
]);

const timestampSchema = z.unknown().transform((value, context) => {
  const parsed = parseTimestamp(value);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "finished_at" });
  return z.NEVER;
});

const newsOrigins = {
  Hankyung: "source.hankyung",
  Maekyung: "source.maekyung",
  Yonhap: "source.yonhap",
  Edaily: "source.edaily",
  SegyeFinance: "source.segye-finance",
  BankOfKorea: "source.bank-of-korea",
  FederalReserve: "source.federal-reserve",
  GoogleNews: "source.google-news",
  YahooFinance: "source.yahoo-finance",
} as const;

const disclosureOrigins = {
  Kind: "source.kind-disclosure",
  Dart: "source.dart-disclosure-api",
  SecEdgar: "source.sec-edgar",
} as const;

const originSchema = z
  .union([
    z.literal("Web").transform(() => "source.web-search" as const),
    z
      .strictObject({
        News: z.enum([
          "Hankyung",
          "Maekyung",
          "Yonhap",
          "Edaily",
          "SegyeFinance",
          "BankOfKorea",
          "FederalReserve",
          "GoogleNews",
          "YahooFinance",
        ]),
      })
      .transform((raw) => newsOrigins[raw.News]),
    z
      .strictObject({ Disclosure: z.enum(["Kind", "Dart", "SecEdgar"]) })
      .transform((raw) => disclosureOrigins[raw.Disclosure]),
  ])
  .transform((key) => uiFreeText(text(key, "ko")));

const sourceTimeSchema = z.union([
  z.null().transform((): undefined => undefined),
  timestampSchema,
]);

const sourceSchema = z.strictObject({
  title: freeTextSchema,
  url: linkSchema,
  kind: z.enum(["web", "news", "disclosure"]),
  cite: z
    .unknown()
    .transform((value) => {
      const parsed = parseEntityId(value);
      return parsed.kind === "ok" ? parsed.value : undefined;
    })
    .optional(),
  origin: originSchema.optional(),
  observed_at: sourceTimeSchema.optional(),
});

const idSchema = z.strictObject({
  kind: z.enum(["Morning", "ClosingReview", "PreUsMarket"]),
  date: z.string().regex(/^\d{4}-\d{2}-\d{2}$/u),
});

const idFromParts = z
  .array(z.string())
  .transform((parts) => parts.join("-"))
  .pipe(chatIdSchema);

const makeId = (parts: Wire): EntityId | undefined => {
  const parsed = idFromParts.safeParse(parts);
  return parsed.success ? parsed.data : undefined;
};

const briefingSchema = z
  .strictObject({
    id: idSchema,
    finished_at: timestampSchema,
    status: z.enum(["completed", "failed"]),
    failure: z.enum([
      "none",
      "ai-unavailable",
      "rate-limited",
      "empty",
      "rejected",
    ]),
    body: freeTextSchema,
    sources: z.array(sourceSchema),
  })
  .transform((raw, context): BriefingSummary => {
    const id = makeId(wire([raw.id.kind, raw.id.date]));
    if (id === undefined) {
      context.addIssue({ code: "custom", message: "id" });
      return z.NEVER;
    }
    const kind: BriefingKind = kindCodes[raw.id.kind];
    if (raw.status === "completed") {
      const sources = raw.sources.flatMap(
        (source, index): readonly BriefingSourceView[] => {
          const sourceId = makeId(
            wire([raw.id.kind, raw.id.date, String(index)]),
          );
          return sourceId === undefined
            ? []
            : [
                {
                  id: sourceId,
                  title: source.title,
                  url: source.url,
                  kind: source.kind,
                  cite: source.cite ?? makeId(wire([String(index + 1)])),
                  publisher: source.origin,
                  observedAt: source.observed_at,
                },
              ];
        },
      );
      return {
        id,
        kind,
        finishedAt: raw.finished_at,
        outcome: { kind: "completed", body: raw.body, sources },
      };
    }
    if (raw.failure === "none") {
      context.addIssue({ code: "custom", message: "failure" });
      return z.NEVER;
    }
    const reason: BriefingFailure = failureCodes[raw.failure];
    return {
      id,
      kind,
      finishedAt: raw.finished_at,
      outcome: { kind: "failed", reason },
    };
  });

const viewSchema = z
  .strictObject({
    latest: z.union([briefingSchema, z.null()]),
    past: z.array(briefingSchema),
  })
  .transform((raw): BriefingsView => ({
    latest: raw.latest ?? undefined,
    past: raw.past,
  }));

export const settleBriefings = (
  outcome: WireOutcome,
): Result<BriefingsView, IpcFailure> => settleWithSchema(outcome, viewSchema);
