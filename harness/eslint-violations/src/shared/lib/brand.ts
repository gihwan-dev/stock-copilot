import { z } from "zod";
type Brand<N extends string> = Readonly<z.core.$brand<N>>;
export type Ticker = string & Brand<"Ticker">;
export type UiText = string & Brand<"UiText">;
export type Plain = string;
export type PlainProps = { readonly title: string };
export type PlainRecord = Readonly<Record<string, Ticker>>;
const plainSource = (): string => "plain";
export const plainValue = plainSource();
export const plainLabel = (value: string): string => value;
const tickerSchema = z.string().brand<"Ticker">();
export const toTicker = (input: unknown): Ticker | undefined => {
  const parsed = tickerSchema.safeParse(input);
  return parsed.success ? parsed.data : undefined;
};
export const asTicker = (input: string): Ticker => input as Ticker;
