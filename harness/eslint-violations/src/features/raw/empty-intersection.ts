import type { Plain } from "../../shared/lib/brand";
export type Fake = Plain & Readonly<Record<never, never>>;
