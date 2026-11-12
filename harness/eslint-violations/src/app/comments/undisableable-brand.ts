import { z } from "zod";
// eslint-disable-next-line local/no-brand-creation -- 끌 수 없는 규칙
export const tickerSchema = z.string().brand<"Ticker">();
