import { z } from "zod";
import {
  externalUrlSchema,
  chatIdSchema,
  freeTextSchema,
  parseEntityId,
  type Result,
  type EntityId,
} from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import { settleWithSchema, wireOutcome, type IpcFailure } from "./schema";
import { wireTimestamp } from "./wire-values";

const source = z.object({
  title: freeTextSchema,
  url: z
    .string()
    .nullable()
    .transform((value) => {
      const parsed = externalUrlSchema.safeParse(value);
      return parsed.success ? parsed.data : null;
    }),
  observedAt: wireTimestamp.nullable(),
});

const section = z.object({
  status: z.enum(["ready", "empty", "failed", "refreshing"]),
  asOf: wireTimestamp.nullable(),
  entries: z.array(
    z.object({
      symbol: chatIdSchema.nullable(),
      displayName: freeTextSchema.nullable().default(null),
      detail: z.enum(["available", "unavailable"]).default("unavailable"),
      body: freeTextSchema,
    }),
  ),
  sources: z.array(source).transform((sources) =>
    sources.flatMap((item, index) => {
      const number = parseEntityId(String(index + 1));
      const label = freeTextSchema.safeParse(`[${String(index + 1)}]`);
      return number.kind === "ok" && label.success
        ? [
            {
              ...item,
              number: number.value,
              label: label.data,
            },
          ]
        : [];
    }),
  ),
});

const dashboard = z.object({
  holdings: section,
  candidates: section,
  economy: section,
});
export type ResearchSection = Readonly<z.infer<typeof section>>;
export type ResearchSource = Readonly<z.infer<typeof source>> & {
  readonly number: EntityId;
  readonly label: z.infer<typeof freeTextSchema>;
};
type Research = {
  readonly holdings: ResearchSection;
  readonly candidates: ResearchSection;
  readonly economy: ResearchSection;
};
export const getDashboardResearch = (): Promise<Result<Research, IpcFailure>> =>
  settleCall(commands.getDashboardResearch(), (result) =>
    settleWithSchema(wireOutcome(result), dashboard),
  );
