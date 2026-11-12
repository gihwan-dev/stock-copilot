import type { z } from "zod";
import type { Ticker } from "../../shared/lib/brand";
export type Forged = Ticker & z.core.$brand<"Forged">;
