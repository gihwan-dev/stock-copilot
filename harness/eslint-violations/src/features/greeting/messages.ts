import type { UiText } from "../../shared/lib/index";
type Locale = "ko";
const catalog = {
  "greeting.title": { ko: "내 현황" as UiText },
  "greeting.empty": { ko: ("보유 " + `종목 없음`) as UiText },
} as const;
export type GreetingKey = keyof typeof catalog;
export const greetingText = (key: GreetingKey, locale: Locale): UiText =>
  catalog[key][locale];
