import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  title: { ko: "앱 정보" as UiText },
} as const;

export type AboutMessageKey = keyof typeof catalog;

export const aboutText = (key: AboutMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
