import { z } from "zod";
export const tickerSchema = z.string().brand<"Ticker">();
