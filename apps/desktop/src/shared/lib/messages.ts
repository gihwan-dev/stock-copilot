import type { UiText } from "./brand";

export type Locale = "ko";

const catalog = {
  "ds.chartMinimum": { ko: "최소" as UiText },
  "ds.chartMaximum": { ko: "최대" as UiText },
  "ds.chartExplore": { ko: "차트 항목 살펴보기" as UiText },
  "ds.chartTrendOrder": {
    ko: "화살표 키로 항목을 옮겨 살펴볼 수 있어요" as UiText,
  },
  "ds.chartDonutOrder": {
    ko: "위쪽부터 시계 방향으로 아래 목록의 순서예요. 값이 0인 항목은 제외해요" as UiText,
  },
  "ds.chartTreemapOrder": {
    ko: "왼쪽부터 아래 목록의 순서예요. 값이 0인 항목은 제외해요" as UiText,
  },
  "ds.chartInvalid": { ko: "차트를 그릴 수 없어요" as UiText },
  "ds.chartSeparator": { ko: " · " as UiText },
  "number.jo": { ko: "조" as UiText },
  "number.empty": { ko: "" as UiText },
  "number.invalid": { ko: "값을 표시할 수 없음" as UiText },
  "number.space": { ko: " " as UiText },
  "number.openRate": { ko: " (" as UiText },
  "number.closeRate": { ko: ")" as UiText },
  "number.won": { ko: "원" as UiText },
  "number.shares": { ko: "주" as UiText },
  "number.man": { ko: "만" as UiText },
  "number.eok": { ko: "억" as UiText },
  "number.unavailable": { ko: "—" as UiText },
  "number.up": { ko: "상승" as UiText },
  "number.down": { ko: "하락" as UiText },
  "number.flat": { ko: "보합" as UiText },
  "number.upSymbol": { ko: "▲" as UiText },
  "number.downSymbol": { ko: "▼" as UiText },
  "number.flatSymbol": { ko: "–" as UiText },
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
