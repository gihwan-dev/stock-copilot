import type { UiText } from "./brand";

export type Locale = "ko";

const catalog = {
  "app.title": { ko: "주식 코파일럿" },
} as const;

export type MessageKey = keyof typeof catalog;

export const text = (key: MessageKey, locale: Locale): UiText =>
  catalog[key][locale] as UiText;
