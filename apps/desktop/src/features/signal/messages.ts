import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  title: { ko: "신호" as UiText },
  loading: { ko: "신호를 불러오는 중이에요" as UiText },
  emptyTitle: { ko: "지금은 신호가 없어요" as UiText },
  emptyBody: {
    ko: "종목 분류와 근거 자료를 확인하고 매수·매도 조건을 충족하면 신호가 표시돼요." as UiText,
  },
  errorTitle: { ko: "신호를 불러오지 못했어요" as UiText },
  retry: { ko: "다시 시도" as UiText },
  buy: { ko: "매수 신호" as UiText },
  sell: { ko: "매도 신호" as UiText },
  factsTitle: { ko: "앱이 계산한 값" as UiText },
  reasonsTitle: { ko: "AI가 낸 근거" as UiText },
  price: { ko: "현재가" as UiText },
  priceUnknown: { ko: "시세를 받지 못했어요" as UiText },
  sizingPending: {
    ko: "수량과 금액은 주문할 때 최신 호가로 계산해요." as UiText,
  },
  quantity: { ko: "수량" as UiText },
  amount: { ko: "금액" as UiText },
  costs: { ko: "예상 수수료·세금" as UiText },
  weightAfter: { ko: "주문 후 비중" as UiText },
  aiProbability: { ko: "AI가 낸 확률" as UiText },
  trackedProbability: { ko: "실제 적중률을 반영한 확률" as UiText },
  threshold: { ko: "기준 확률" as UiText },
  binHitRate: { ko: "이 확률 구간의 과거 적중률" as UiText },
  binHitRateNone: { ko: "아직 판정된 기록이 없어요" as UiText },
  binCount: { ko: "판정 건수" as UiText },
  rationale: { ko: "근거" as UiText },
  counterargument: { ko: "반대 논거" as UiText },
  sources: { ko: "출처" as UiText },
  noSources: { ko: "출처 없음" as UiText },
  externalOnly: {
    ko: "근거가 뉴스·웹·공시 같은 외부 출처뿐이에요. 직접 확인해 주세요." as UiText,
  },
  sourceWeb: { ko: "웹" as UiText },
  sourceNews: { ko: "뉴스" as UiText },
  sourceDisclosure: { ko: "공시" as UiText },
  marketClosed: { ko: "장 열리면 확인" as UiText },
  marketClosedBody: {
    ko: "장이 열리면 그때 가격으로 다시 확인해 주문 카드로 바꿔 드려요." as UiText,
  },
  order: { ko: "주문" as UiText },
  outcomeRequested: {
    ko: "주문을 보냈어요. 체결은 토스증권 앱에서 확인할 수 있어요." as UiText,
  },
  outcomeUnknown: {
    ko: "주문 결과를 아직 확인하지 못했어요. 토스증권 앱의 주문 내역을 확인해 주세요." as UiText,
  },
  outcomeDenied: { ko: "본인 확인을 하지 않아 주문하지 않았어요." as UiText },
  outcomeSwitchOff: {
    ko: "주문 기능이 꺼져 있어요. 설정에서 주문 켜기를 켜 주세요." as UiText,
  },
  outcomeHourly: {
    ko: "한 시간에 낼 수 있는 주문 수를 넘었어요. 잠시 뒤에 다시 해 주세요." as UiText,
  },
  outcomeClosed: {
    ko: "지금은 장이 닫혀 있어요. 장이 열리면 다시 해 주세요." as UiText,
  },
  outcomeBusy: {
    ko: "다른 확인 창이 열려 있어요. 그 창을 닫고 다시 해 주세요." as UiText,
  },
  outcomeCooldown: {
    ko: "방금 취소한 주문이라 잠시 뒤에 다시 할 수 있어요." as UiText,
  },
  outcomeConsumed: { ko: "이미 보낸 주문이에요." as UiText },
  outcomeGone: {
    ko: "이 제안은 더 이상 유효하지 않아요. 새 신호가 오면 다시 알려 드릴게요." as UiText,
  },
  outcomeChanged: {
    ko: "가격이나 잔고가 바뀌어 주문하지 않았어요. 카드를 새로 확인해 주세요." as UiText,
  },
  outcomeHighValue: {
    ko: "토스증권이 고액 주문 확인을 요구해 주문하지 못했어요. 토스증권 앱에서 직접 주문해 주세요." as UiText,
  },
  outcomeBroker: {
    ko: "토스증권이 주문을 받지 않았어요. 토스증권 앱에서 계좌와 종목 상태를 확인해 주세요." as UiText,
  },
  outcomeUnavailable: {
    ko: "지금은 주문할 수 없어요. 잠시 뒤에 다시 해 주세요." as UiText,
  },
  outcomeOther: {
    ko: "주문하지 못했어요. 잠시 뒤에 다시 해 주세요." as UiText,
  },

  switchOff: {
    ko: '주문 기능이 꺼져 있어요. 설정 > 기본의 "주문 켜기"를 켜면 이 카드에서 주문할 수 있어요.' as UiText,
  },
  confirmSkip: { ko: "넘기기 확인" as UiText },
  skip: { ko: "넘기기" as UiText },
  skipReason: { ko: "넘기는 이유 (선택)" as UiText },
  skipReasonHint: {
    ko: "기록으로만 남고 다음 확률 추정에는 쓰이지 않아요." as UiText,
  },
} as const;

export type SignalMessageKey = keyof typeof catalog;

export const signalText = (key: SignalMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
