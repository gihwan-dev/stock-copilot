import { z } from "zod";
const method = "brand";
export const tickerSchema = z.string()[method]<"Ticker">();
