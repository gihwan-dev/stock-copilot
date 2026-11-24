import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  "risk.editTitle": { ko: "투자 성향과 주문 설정 바꾸기" as UiText },
  "risk.profileTitle": { ko: "투자 성향 바꾸기" as UiText },
  "risk.orderTitle": { ko: "고급 · 주문" as UiText },
  "risk.loading": { ko: "설정을 불러오는 중이에요" as UiText },
  "risk.loadFailed": { ko: "설정을 불러오지 못했어요" as UiText },
  "risk.retry": { ko: "다시 시도" as UiText },
  "risk.defaults": {
    ko: "아직 저장한 값이 없어 기본값을 보여 드려요." as UiText,
  },
  "risk.horizon": { ko: "기간" as UiText },
  "risk.horizon.few-days": { ko: "며칠" as UiText },
  "risk.horizon.weeks-to-months": { ko: "몇 주~몇 달" as UiText },
  "risk.horizon.half-year-or-more": { ko: "반년 이상" as UiText },
  "risk.horizon.hours": { ko: "몇 시간" as UiText },
  "risk.appetite": { ko: "위험 성향" as UiText },
  "risk.appetite.conservative": { ko: "보수" as UiText },
  "risk.appetite.neutral": { ko: "중립" as UiText },
  "risk.appetite.aggressive": { ko: "공격" as UiText },
  "risk.instrumentCap": { ko: "종목 비중 상한" as UiText },
  "risk.sectorCap": { ko: "섹터 비중 상한" as UiText },
  "risk.cap.10": { ko: "10퍼센트" as UiText },
  "risk.cap.15": { ko: "15퍼센트" as UiText },
  "risk.cap.20": { ko: "20퍼센트" as UiText },
  "risk.cap.25": { ko: "25퍼센트" as UiText },
  "risk.cap.30": { ko: "30퍼센트" as UiText },
  "risk.cap.40": { ko: "40퍼센트" as UiText },
  "risk.cap.50": { ko: "50퍼센트" as UiText },
  "risk.excluded": { ko: "신호에서 뺄 종목 유형" as UiText },
  "risk.type.etf": { ko: "ETF" as UiText },
  "risk.type.etn": { ko: "ETN" as UiText },
  "risk.type.preferred": { ko: "우선주" as UiText },
  "risk.type.spac": { ko: "스팩" as UiText },
  "risk.type.reit": { ko: "리츠" as UiText },
  "risk.confirmNote": {
    ko: "몇 시간 단위 기간, 비중 상한 완화, 더 공격적인 위험 성향은 저장할 때 확인 창이 한 번 더 떠요." as UiText,
  },
  "risk.marketOrders": { ko: "시장가 허용" as UiText },
  "risk.hoursHorizon": { ko: "몇 시간 단위 기간 허용" as UiText },
  "risk.cardExpiry": { ko: "주문 카드 만료 시간" as UiText },
  "risk.expiry.5": { ko: "5분" as UiText },
  "risk.expiry.10": { ko: "10분" as UiText },
  "risk.expiry.15": { ko: "15분" as UiText },
  "risk.expiry.30": { ko: "30분" as UiText },
  "risk.orderCount": { ko: "하루 주문 횟수 상한" as UiText },
  "risk.count.off": { ko: "두지 않음" as UiText },
  "risk.count.3": { ko: "3번" as UiText },
  "risk.count.5": { ko: "5번" as UiText },
  "risk.count.10": { ko: "10번" as UiText },
  "risk.save": { ko: "저장" as UiText },
  "risk.saved": { ko: "저장했어요" as UiText },
  "risk.unchanged": { ko: "바뀐 값이 없어요" as UiText },
  "risk.declined": {
    ko: "확인 창에서 취소되어 이전 값을 그대로 둬요." as UiText,
  },
  "risk.unavailable": {
    ko: "지금은 확인 창을 열 수 없어 이전 값을 그대로 둬요." as UiText,
  },
  "risk.saveFailed": {
    ko: "저장하지 못했어요. 다시 시도해 주세요." as UiText,
  },
} as const;

export type RiskMessageKey = keyof typeof catalog;

export const riskText = (key: RiskMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
