import { z } from "zod";
import { err, ok, type Result } from "./result";

type Brand<N extends string> = Readonly<z.core.$brand<N>>;

export type EntityId = string & Brand<"EntityId">;
export type Ticker = string & Brand<"Ticker">;
export type Timestamp = string & Brand<"Timestamp">;
export type Count = number & Brand<"Count">;
export type UiText = string & Brand<"UiText">;

export type BrandError = { readonly kind: "invalid-brand" };

const entityIdSchema = z.string().min(1).brand<"EntityId">();
const tickerSchema = z
  .string()
  .regex(/^[0-9A-Z]{6}$/u)
  .brand<"Ticker">();
const timestampSchema = z.iso.datetime({ offset: true }).brand<"Timestamp">();
export const countSchema = z.number().int().nonnegative().brand<"Count">();

const fromParse = <T>(
  parsed: Readonly<z.ZodSafeParseResult<T>>,
): Result<T, BrandError> =>
  parsed.success ? ok(parsed.data) : err({ kind: "invalid-brand" });

export const parseEntityId = (input: unknown): Result<EntityId, BrandError> =>
  fromParse(entityIdSchema.safeParse(input));

export const parseTicker = (input: unknown): Result<Ticker, BrandError> =>
  fromParse(tickerSchema.safeParse(input));

export const parseTimestamp = (input: unknown): Result<Timestamp, BrandError> =>
  fromParse(timestampSchema.safeParse(input));

export const parseCount = (input: unknown): Result<Count, BrandError> =>
  fromParse(countSchema.safeParse(input));

export type InputValue = string & Brand<"InputValue">;
const inputValueSchema = z.string().brand<"InputValue">();
export const emptyInputValue = "" as InputValue;
export const parseInputValue = (
  input: unknown,
): Result<InputValue, BrandError> =>
  fromParse(inputValueSchema.safeParse(input));

export type ChartDecimal = string & Brand<"ChartDecimal">;
export type ChartCoordinate = number & Brand<"ChartCoordinate">;
export type ChartPath = string & Brand<"ChartPath">;
const chartDecimalSchema = z
  .string()
  .regex(/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/u)
  .brand<"ChartDecimal">();
export const parseChartDecimal = (
  input: unknown,
): Result<ChartDecimal, BrandError> =>
  fromParse(chartDecimalSchema.safeParse(input));
export const chartCoordinate = (input: number): ChartCoordinate =>
  input as ChartCoordinate;
export const chartPath = (input: string): ChartPath => input as ChartPath;
export type FormattedNumber = string & Brand<"FormattedNumber">;

export type FreeText = string & Brand<"FreeText">;
export const freeTextSchema = z.string().brand<"FreeText">();
export const parseFreeText = (input: unknown): Result<FreeText, BrandError> =>
  fromParse(freeTextSchema.safeParse(input));
export const emptyFreeText = "" as FreeText;
export const appendFreeText = (head: FreeText, tail: FreeText): FreeText =>
  (head + tail) as FreeText;
export type FreeTextFold =
  | { readonly kind: "whole" }
  | { readonly kind: "folded"; readonly head: FreeText };
const STRUCTURED_TEXT = /^\s*(?:\||#{1,6}\s|[-*•]\s|\d+[.)]\s)/mu;
const SENTENCE_BREAK = /(?<=[.?!。])\s+/u;
export const foldFreeText = (text: FreeText, visible: 4): FreeTextFold => {
  if (STRUCTURED_TEXT.test(text)) return { kind: "whole" };
  const sentences = text.trim().split(SENTENCE_BREAK);
  if (sentences.length <= visible) return { kind: "whole" };
  return {
    kind: "folded",
    head: sentences.slice(0, visible).join(" ") as FreeText,
  };
};
export type ExternalUrl = string & Brand<"ExternalUrl">;
export const externalUrlSchema = z
  .url({ protocol: /^https$/u })
  .brand<"ExternalUrl">();
export const chatIdSchema = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[0-9A-Za-z_.-]+$/u)
  .brand<"EntityId">();
export const parseExternalUrl = (
  input: unknown,
): Result<ExternalUrl, BrandError> =>
  fromParse(externalUrlSchema.safeParse(input));

export type CanvasColor = string & Brand<"CanvasColor">;
const canvasColorSchema = z.string().trim().min(1).brand<"CanvasColor">();
export const parseCanvasColor = (
  input: unknown,
): Result<CanvasColor, BrandError> =>
  fromParse(canvasColorSchema.safeParse(input));

export type PanelShare = string & Brand<"PanelShare">;
export type PanelBounds = {
  readonly min: number;
  readonly max: number;
  readonly initial: number;
};
const panelShareSchema = z.coerce.number().positive();
const shareText = (share: number): PanelShare =>
  share.toFixed(1).replace(/\.0$/u, "") as PanelShare;
export const clampPanelShare = (
  input: unknown,
  bounds: PanelBounds,
): PanelShare => {
  const parsed = panelShareSchema.safeParse(
    typeof input === "string" && input.trim() === "" ? undefined : input,
  );
  if (!parsed.success) return shareText(bounds.initial);
  if (parsed.data < bounds.min) return shareText(bounds.min);
  if (parsed.data > bounds.max) return shareText(bounds.max);
  return shareText(parsed.data);
};
