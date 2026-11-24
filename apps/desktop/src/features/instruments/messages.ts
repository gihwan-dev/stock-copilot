import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  listLoading: { ko: "종목을 불러오는 중이에요" as UiText },
  listEmpty: { ko: "표시할 종목이 없어요" as UiText },
  listError: { ko: "종목 목록을 불러오지 못했어요" as UiText },
  retry: { ko: "다시 시도" as UiText },
  holding: { ko: "보유" as UiText },
  watchlist: { ko: "관심" as UiText },
  open: { ko: "상세 보기" as UiText },
  addLabel: { ko: "관심 종목 추가 (종목 코드)" as UiText },
  addAction: { ko: "추가" as UiText },
  addInvalid: { ko: "종목 코드 6자리를 넣어 주세요." as UiText },
  addFailed: { ko: "지금은 추가하지 못했어요. 다시 시도해 주세요." as UiText },
  removeAction: { ko: "관심 종목에서 빼기" as UiText },
  detailLoading: { ko: "종목 정보를 불러오는 중이에요" as UiText },
  detailEmpty: { ko: "이 종목의 시세가 아직 없어요" as UiText },
  detailError: { ko: "종목 정보를 불러오지 못했어요" as UiText },
  close: { ko: "닫기" as UiText },
  quoteTitle: { ko: "시세" as UiText },
  price: { ko: "현재가" as UiText },
  today: { ko: "오늘 등락" as UiText },
  quotedAt: { ko: "시세 시각" as UiText },
  chartTitle: { ko: "일봉 차트" as UiText },
  chartLabel: { ko: "일봉 차트" as UiText },
  chartEmpty: { ko: "차트로 그릴 일봉이 없어요" as UiText },
  holdingTitle: { ko: "내 보유" as UiText },
  holdingNone: { ko: "이 종목은 보유하고 있지 않아요" as UiText },
  quantity: { ko: "수량" as UiText },
  value: { ko: "평가 금액" as UiText },
  profit: { ko: "평가 손익" as UiText },
  thesisTitle: { ko: "논거 노트" as UiText },
  thesisLoading: { ko: "논거 노트를 불러오는 중이에요" as UiText },
  thesisEmpty: { ko: "아직 적힌 논거가 없어요" as UiText },
  thesisError: { ko: "논거 노트를 불러오지 못했어요" as UiText },
  thesisCurrent: { ko: "현재 노트" as UiText },
  thesisHistory: { ko: "바뀐 기록" as UiText },
  confirmed: { ko: "확인됨" as UiText },
  needsReview: { ko: "확인 필요" as UiText },
} as const;

export type InstrumentsMessageKey = keyof typeof catalog;

export const instrumentsText = (
  key: InstrumentsMessageKey,
  locale: Locale,
): UiText => catalog[key][locale];
