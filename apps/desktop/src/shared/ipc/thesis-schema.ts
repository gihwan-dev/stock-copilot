import { z } from "zod";
import {
  emptyFreeText,
  freeTextSchema,
  parseEntityId,
  text,
  type EntityId,
  type Result,
} from "../lib";
import type { ThesisClaim, ThesisRevision, ThesisView } from "./estimate-views";
import { settleWithSchema, type IpcFailure, type WireOutcome } from "./schema";
import { wireTimestamp } from "./wire-values";

const entityId = z.unknown().transform((value, context): EntityId => {
  const parsed = parseEntityId(value);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "id" });
  return z.NEVER;
});

const changeKeys = {
  "claim-added": "thesis.claim-added",
  "claim-judged": "thesis.claim-judged",
  "criterion-added": "thesis.criterion-added",
  "criterion-judged": "thesis.criterion-judged",
  "user-confirmed": "thesis.user-confirmed",
  "user-rejected": "thesis.user-rejected",
} as const;

const verdictKeys = {
  intact: "verdict.intact",
  weakened: "verdict.weakened",
  broken: "verdict.broken",
  unverifiable: "verdict.unverifiable",
} as const;

const claimSchema = z
  .strictObject({
    id: entityId,
    text: freeTextSchema,
    status: z.enum(["confirmed", "needs-review"]),
  })
  .transform((raw): ThesisClaim => ({
    id: raw.id,
    text: raw.text,
    status: raw.status,
  }));

const revisionSchema = z
  .strictObject({
    sequence: entityId,
    at: z.union([wireTimestamp, z.null()]),
    change: z.enum([
      "claim-added",
      "claim-judged",
      "criterion-added",
      "criterion-judged",
      "user-confirmed",
      "user-rejected",
    ]),
    subject: z.union([freeTextSchema, z.null()]),
    verdict: z.union([
      z.enum(["intact", "weakened", "broken", "unverifiable"]),
      z.null(),
    ]),
  })
  .transform((raw): ThesisRevision => {
    const label = text(changeKeys[raw.change], "ko");
    const verdict =
      raw.verdict === null ? "" : ` · ${text(verdictKeys[raw.verdict], "ko")}`;
    const subject = raw.subject === null ? "" : ` — ${raw.subject}`;
    const summary = freeTextSchema.safeParse(`${label}${verdict}${subject}`);
    return {
      id: raw.sequence,
      at: raw.at ?? undefined,
      summary: summary.success ? summary.data : emptyFreeText,
    };
  });

const viewSchema = z
  .strictObject({
    claims: z.array(claimSchema),
    history: z.array(revisionSchema),
  })
  .transform((raw): ThesisView => ({
    claims: raw.claims,
    history: raw.history,
  }));

export const settleThesis = (
  outcome: WireOutcome,
): Result<ThesisView, IpcFailure> => settleWithSchema(outcome, viewSchema);
