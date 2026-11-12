import { z } from "zod";
import type { Ticker } from "../../shared/lib/brand";
export const tickerSchema = z.custom<Ticker>();
