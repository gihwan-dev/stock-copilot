import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  overviewSection: { ko: "현황" as UiText },
  totalTitle: { ko: "총평가" as UiText },
  totalApproximate: { ko: "근사값" as UiText },
  totalHint: {
    ko: "보유 평가액과 매수가능금액을 원화로 환산해 더한 근사값이에요." as UiText,
  },
  exchangeRateNote: { ko: "매매기준율 환산 시각" as UiText },
  recordedAt: { ko: "기준 시각" as UiText },
  storedNotice: {
    ko: "인터넷에 연결되지 않아 저장된 자료를 보여 드리고 있어요." as UiText,
  },
  todayTitle: { ko: "오늘 손익" as UiText },
  todayPending: { ko: "오늘 손익은 아직 받지 못했어요." as UiText },
  totalPartial: {
    ko: "환율을 아직 받지 못해 달러 자산은 총평가에서 빠져 있어요." as UiText,
  },
  notObserved: { ko: "확인 안 됨" as UiText },
  cashKrw: { ko: "원화 매수가능금액" as UiText },
  cashUsd: { ko: "달러 매수가능금액" as UiText },
  errorAccessBody: {
    ko: "토스증권 연결을 확인하지 못했어요. 설정에서 연결 상태를 확인해 주세요." as UiText,
  },
  currencyTitle: { ko: "통화별 평가" as UiText },
  currencyColumn: { ko: "통화" as UiText },
  holdingsColumn: { ko: "보유 평가" as UiText },
  buyingPowerColumn: { ko: "매수가능금액" as UiText },
  currencyKrw: { ko: "원화" as UiText },
  currencyUsd: { ko: "달러" as UiText },
  holdingsTitle: { ko: "보유 종목" as UiText },
  nameColumn: { ko: "종목" as UiText },
  tickerColumn: { ko: "코드" as UiText },
  quantityColumn: { ko: "수량" as UiText },
  valueColumn: { ko: "평가금액" as UiText },
  profitColumn: { ko: "손익" as UiText },
  holdingsEmpty: { ko: "보유 종목이 없어요" as UiText },
  allocationSection: { ko: "자산배분과 쏠림" as UiText },
  instrumentTitle: { ko: "종목별 비중" as UiText },
  sectorTitle: { ko: "섹터별 비중" as UiText },
  allocationNameColumn: { ko: "이름" as UiText },
  weightColumn: { ko: "비중" as UiText },
  levelColumn: { ko: "상한" as UiText },
  levelNormal: { ko: "여유" as UiText },
  levelWarning: { ko: "주의" as UiText },
  levelOverCap: { ko: "초과" as UiText },
  balanced: { ko: "한쪽으로 쏠리지 않았어요." as UiText },
  concentrated: {
    ko: "몇 종목에 비중이 몰려 있어요. 종목별 비중을 확인해 보세요." as UiText,
  },
  allocationEmpty: { ko: "아직 자산배분을 계산할 자료가 없어요" as UiText },
  emptyBody: {
    ko: "토스증권 계좌 자료를 받으면 여기에 현황이 보여요." as UiText,
  },
  loadingTitle: { ko: "현황을 불러오는 중이에요" as UiText },
  errorTitle: { ko: "현황을 불러오지 못했어요" as UiText },
  allocationErrorTitle: { ko: "자산배분을 불러오지 못했어요" as UiText },
  retry: { ko: "다시 시도" as UiText },
  openTransactions: { ko: "거래내역" as UiText },
} as const;

export type PortfolioMessageKey = keyof typeof catalog;

export const portfolioText = (
  key: PortfolioMessageKey,
  locale: Locale,
): UiText => catalog[key][locale];
