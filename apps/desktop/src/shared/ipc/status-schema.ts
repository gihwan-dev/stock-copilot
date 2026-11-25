import { z } from "zod";
import {
  emptyFreeText,
  parseEntityId,
  parseFreeText,
  parseTimestamp,
  text,
  type FreeText,
  type MessageKey,
  type Result,
  type Timestamp,
  type UiText,
} from "../lib";
import { settleWithSchema, type IpcFailure } from "./schema";
import type { SelfCheckView, StatusView } from "./screen-sources";

type CommandOutcome =
  | { readonly status: "ok"; readonly data: unknown }
  | { readonly status: "error"; readonly error: unknown };

const sourceKeys = {
  "kind-disclosure": "source.kind-disclosure",
  "dart-disclosure-api": "source.dart-disclosure-api",
  "dart-disclosure-rss": "source.dart-disclosure-rss",
  "dart-fundamentals": "source.dart-fundamentals",
  "kind-corp-list": "source.kind-corp-list",
  hankyung: "source.hankyung",
  maekyung: "source.maekyung",
  yonhap: "source.yonhap",
  edaily: "source.edaily",
  "google-news": "source.google-news",
  "web-search": "source.web-search",
  "yahoo-finance": "source.yahoo-finance",
  "sec-edgar": "source.sec-edgar",
  "bank-of-korea": "source.bank-of-korea",
  "federal-reserve": "source.federal-reserve",
  "us-treasury": "source.us-treasury",
  fred: "source.fred",
} as const satisfies Record<string, MessageKey>;

type SourceCode = keyof typeof sourceKeys;

const sourceCodes: readonly [SourceCode, ...SourceCode[]] = [
  "kind-disclosure",
  "dart-disclosure-api",
  "dart-disclosure-rss",
  "dart-fundamentals",
  "kind-corp-list",
  "hankyung",
  "maekyung",
  "yonhap",
  "edaily",
  "google-news",
  "web-search",
  "yahoo-finance",
  "sec-edgar",
  "bank-of-korea",
  "federal-reserve",
  "us-treasury",
  "fred",
];

const asFreeText = (value: UiText): FreeText => {
  const parsed = parseFreeText(value);
  return parsed.kind === "ok" ? parsed.value : emptyFreeText;
};

const timestampSchema = z.unknown().transform((value, context) => {
  const parsed = parseTimestamp(value);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "timestamp" });
  return z.NEVER;
});

const optionalTimestamp = z
  .union([timestampSchema, z.null()])
  .transform((value): Timestamp | undefined => value ?? undefined);

const selfCheckSchema = z
  .strictObject({
    problem: z.enum([
      "none",
      "toss-key-invalid",
      "toss-key-permission",
      "toss-account-missing",
      "ip-not-allowed",
      "toss-key-shared",
      "claude-missing",
      "claude-signed-out",
      "order-risk-notice",
      "order-exchange-setting",
      "order-dollar-shortage",
    ]),
  })
  .transform((raw): SelfCheckView =>
    raw.problem === "none"
      ? { kind: "clear" }
      : { kind: "problem", problem: raw.problem },
  );

const statusSchema = z
  .strictObject({
    ai_runtime: z.enum(["ready", "missing"]),
    ai_resumes_at: optionalTimestamp,
    sources: z.array(
      z.strictObject({
        source: z.enum(sourceCodes),
        last_success: optionalTimestamp,
      }),
    ),
  })
  .transform((raw, context): StatusView => {
    const sources = raw.sources.map((row) => {
      const id = parseEntityId(row.source);
      return id.kind === "ok"
        ? {
            id: id.value,
            name: asFreeText(text(sourceKeys[row.source], "ko")),
            lastSuccess: row.last_success,
          }
        : undefined;
    });
    const kept = sources.filter((row) => row !== undefined);
    if (kept.length !== sources.length) {
      context.addIssue({ code: "custom", message: "source" });
      return z.NEVER;
    }
    return {
      records: [],
      aiRuntime:
        raw.ai_runtime === "ready"
          ? { kind: "ready" }
          : {
              kind: "unavailable",
              reason: asFreeText(text("status.claudeMissing", "ko")),
            },
      aiLimit:
        raw.ai_resumes_at === undefined
          ? { kind: "available" }
          : { kind: "waiting", resumesAt: raw.ai_resumes_at },
      sources: kept,
      aiUsage: [],
    };
  });

export const settleSelfCheck = (
  outcome: CommandOutcome,
): Result<SelfCheckView, IpcFailure> =>
  settleWithSchema(outcome, selfCheckSchema);

export const settleStatus = (
  outcome: CommandOutcome,
): Result<StatusView, IpcFailure> => settleWithSchema(outcome, statusSchema);
