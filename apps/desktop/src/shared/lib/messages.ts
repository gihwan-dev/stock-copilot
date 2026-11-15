import type { UiText } from "./brand";

export type Locale = "ko";

const catalog = {
  "ds.chartInvalid": { ko: "차트 값을 읽을 수 없어요" as UiText },
  "ds.chartSeparator": { ko: " · " as UiText },
  "ds.badgeNeutral": { ko: "보유 종목" as UiText },
  "ds.badgePrimary": { ko: "앱 계산" as UiText },
  "ds.badgeAlert": { ko: "값 오류" as UiText },
  "ds.statusWarning": { ko: "자료가 오래됐어요" as UiText },
  "ds.statusError": { ko: "토스에 연결하지 못했어요" as UiText },
  "ds.statusInfo": { ko: "오늘은 휴장이에요" as UiText },
  "ds.retry": { ko: "다시 시도" as UiText },
  "ds.close": { ko: "닫기" as UiText },
  "ds.tableEmpty": { ko: "표시할 항목이 없어요" as UiText },
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
