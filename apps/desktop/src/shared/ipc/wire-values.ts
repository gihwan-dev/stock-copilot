import { z } from "zod";
import {
  emptyFreeText,
  parseFreeText,
  parseMoney,
  parsePercentDisplay,
  parseQuantity,
  parseTimestamp,
  type FreeText,
  type Money,
  type PercentDisplay,
  type Quantity,
  type Timestamp,
  type UiText,
} from "../lib";

export const wireTimestamp = z
  .unknown()
  .transform((value, context): Timestamp => {
    const parsed = parseTimestamp(value);
    if (parsed.kind === "ok") return parsed.value;
    context.addIssue({ code: "custom", message: "timestamp" });
    return z.NEVER;
  });

export const wireMoney = z
  .strictObject({ amount: z.string(), currency: z.enum(["KRW", "USD"]) })
  .transform((raw, context): Money => {
    const parsed = parseMoney(raw.amount, raw.currency);
    if (parsed.kind === "ok") return parsed.value;
    context.addIssue({ code: "custom", message: "money" });
    return z.NEVER;
  });

export const wirePercent = z
  .unknown()
  .transform((raw, context): PercentDisplay => {
    const parsed =
      typeof raw === "string" ? parsePercentDisplay(raw) : undefined;
    if (parsed?.kind === "ok") return parsed.value;
    context.addIssue({ code: "custom", message: "percent" });
    return z.NEVER;
  });

export const wireQuantity = z.unknown().transform((raw, context): Quantity => {
  const parsed = typeof raw === "string" ? parseQuantity(raw) : undefined;
  if (parsed?.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "quantity" });
  return z.NEVER;
});

export const uiFreeText = (value: UiText): FreeText => {
  const parsed = parseFreeText(value);
  return parsed.kind === "ok" ? parsed.value : emptyFreeText;
};
