import type { UiText } from "../../shared/lib/index";
type Locale = "ko";
const catalog = {
  "boundary.title": { ko: "내 현황" as UiText },
  "boundary.empty": { ko: ("보유 " + `종목 없음`) as UiText },
} as const;
export type BoundaryKey = keyof typeof catalog;
export const boundaryText = (key: BoundaryKey, locale: Locale): UiText =>
  catalog[key][locale];
