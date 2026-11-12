import type { UiText } from "../../shared/lib/brand";
type Locale = "ko";
const catalog = {
  "home.title": { ko: "내 현황" as UiText },
  "home.empty": { ko: ("보유 " + `종목 없음`) as UiText },
} as const;
export type HomeKey = keyof typeof catalog;
export const homeText = (key: HomeKey, locale: Locale): UiText =>
  catalog[key][locale];
