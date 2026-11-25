import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  title: { ko: "거래내역" as UiText },
  backToHome: { ko: "현황으로" as UiText },
  tableCaption: { ko: "결제일 기준 거래내역" as UiText },
  dateColumn: { ko: "결제일" as UiText },
  nameColumn: { ko: "종목·내용" as UiText },
  kindColumn: { ko: "구분" as UiText },
  quantityColumn: { ko: "수량" as UiText },
  amountColumn: { ko: "금액" as UiText },
  stateColumn: { ko: "상태" as UiText },
  kindBuy: { ko: "매수" as UiText },
  kindSell: { ko: "매도" as UiText },
  kindDividend: { ko: "배당" as UiText },
  kindDeposit: { ko: "입금" as UiText },
  kindWithdrawal: { ko: "출금" as UiText },
  kindFee: { ko: "수수료" as UiText },
  kindOther: { ko: "기타" as UiText },
  vanished: { ko: "토스에서 사라짐" as UiText },
  vanishedNotice: {
    ko: "토스 거래내역에서 더 이상 보이지 않는 행은 지우지 않고 '토스에서 사라짐'으로 남겨 둬요." as UiText,
  },
  noQuantity: { ko: "—" as UiText },
  recordedAt: { ko: "기준 시각" as UiText },
  loadingTitle: { ko: "거래내역을 불러오는 중이에요" as UiText },
  emptyTitle: { ko: "아직 거래내역이 없어요" as UiText },
  emptyBody: {
    ko: "토스증권 계좌 자료를 받으면 최근 거래가 여기에 보여요." as UiText,
  },
  errorTitle: { ko: "거래내역을 불러오지 못했어요" as UiText },
  retry: { ko: "다시 시도" as UiText },
} as const;

export type TransactionsMessageKey = keyof typeof catalog;

export const transactionsText = (
  key: TransactionsMessageKey,
  locale: Locale,
): UiText => catalog[key][locale];
