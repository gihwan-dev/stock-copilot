import type { UiText } from "./brand";

export type Locale = "ko";

const catalog = {
  "ds.day": { ko: "하루" as UiText },
  "ds.month": { ko: "한 달" as UiText },
  "ds.year": { ko: "일 년" as UiText },
  "ds.description": { ko: "설명을 확인해 주세요" as UiText },
  "ds.show": { ko: "보기" as UiText },
  "ds.hide": { ko: "가리기" as UiText },
  "ds.paste": { ko: "붙여넣기" as UiText },
  "ds.pasteError": { ko: "붙여넣지 못했어요. 직접 입력해 주세요" as UiText },
  "app.title": { ko: "주식 코파일럿" as UiText },
} as const;

export type MessageKey = keyof typeof catalog;

export const text = (key: MessageKey, locale: Locale): UiText =>
  catalog[key][locale];
