import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  "principles.title": { ko: "투자 원칙" as UiText },
  "principles.loading": { ko: "불러오는 중이에요" as UiText },
  "principles.loadFailed": {
    ko: "투자 원칙을 불러오지 못했어요. 잠시 뒤 다시 열어 주세요." as UiText,
  },
  "principles.body": {
    ko: "적어 둔 원칙은 신호 검사와 AI가 판단할 때 함께 쓰여요." as UiText,
  },
  "principles.textLabel": { ko: "나의 투자 원칙" as UiText },
  "principles.textHint": {
    ko: "예: 실적이 꺾인 종목은 사지 않는다" as UiText,
  },
  "principles.textInvalid": {
    ko: "4000자 안으로 적어 주세요." as UiText,
  },
  "principles.cashLabel": { ko: "현금 최소 비중" as UiText },
  "principles.cash.0": { ko: "두지 않음" as UiText },
  "principles.cash.5": { ko: "총자산의 5 퍼센트" as UiText },
  "principles.cash.10": { ko: "총자산의 10 퍼센트" as UiText },
  "principles.cash.20": { ko: "총자산의 20 퍼센트" as UiText },
  "principles.cash.30": { ko: "총자산의 30 퍼센트" as UiText },
  "principles.concentrationLabel": { ko: "쏠림 경고 기준" as UiText },
  "principles.count.3": { ko: "유효 종목 수 3개 미만이면 경고" as UiText },
  "principles.count.4": { ko: "유효 종목 수 4개 미만이면 경고" as UiText },
  "principles.count.5": { ko: "유효 종목 수 5개 미만이면 경고" as UiText },
  "principles.count.6": { ko: "유효 종목 수 6개 미만이면 경고" as UiText },
  "principles.count.8": { ko: "유효 종목 수 8개 미만이면 경고" as UiText },
  "principles.count.10": { ko: "유효 종목 수 10개 미만이면 경고" as UiText },
  "principles.save": { ko: "저장" as UiText },
  "principles.saved": { ko: "저장했어요" as UiText },
  "principles.saveFailed": {
    ko: "저장하지 못했어요. 다시 시도해 주세요." as UiText,
  },
} as const;

export type PrinciplesMessageKey = keyof typeof catalog;

export const principlesText = (
  key: PrinciplesMessageKey,
  locale: Locale,
): UiText => catalog[key][locale];
