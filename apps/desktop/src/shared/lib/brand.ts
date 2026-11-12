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
const countSchema = z.number().int().nonnegative().brand<"Count">();

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
