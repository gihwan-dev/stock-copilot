import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  appearanceTitle: { ko: "화면 모양" as UiText },
  widgetGap: { ko: "위젯 간격" as UiText },
  widgetPadding: { ko: "안쪽 여백" as UiText },
  widgetRadius: { ko: "카드 모서리" as UiText },
  pixels0: { ko: "0 픽셀" as UiText },
  pixels4: { ko: "4 픽셀" as UiText },
  pixels8: { ko: "8 픽셀" as UiText },
  pixels12: { ko: "12 픽셀" as UiText },
  pixels16: { ko: "16 픽셀" as UiText },
  title: { ko: "설정" as UiText },
  back: { ko: "이전 화면으로" as UiText },
  backToHome: { ko: "현황으로" as UiText },
  basicSection: { ko: "기본 설정" as UiText },
  tossTitle: { ko: "토스 연결" as UiText },
  tossConnected: { ko: "토스증권 계좌가 연결되어 있어요." as UiText },
  tossNeedsKey: { ko: "토스증권 키를 넣으면 연결돼요." as UiText },
  tossPasteAction: { ko: "키 다시 붙여 넣기" as UiText },
  tossPasteHint: {
    ko: "토스증권 WTS의 Open API 설정에서 발급한 키를 붙여 넣어 연결할 수 있어요." as UiText,
  },
  profileTitle: { ko: "투자 성향" as UiText },
  profileDetails: { ko: "투자 성향 자세히 보기" as UiText },
  horizonLabel: { ko: "기간" as UiText },
  horizonHours: { ko: "몇 시간" as UiText },
  horizonFewDays: { ko: "며칠" as UiText },
  horizonWeeksToMonths: { ko: "몇 주~몇 달" as UiText },
  horizonHalfYearOrMore: { ko: "반년 이상" as UiText },
  riskLabel: { ko: "위험 성향" as UiText },
  riskConservative: { ko: "보수" as UiText },
  riskNeutral: { ko: "중립" as UiText },
  riskAggressive: { ko: "공격" as UiText },
  exclusionsLabel: { ko: "제외 유형·섹터" as UiText },
  exclusionsNone: { ko: "제외 없음" as UiText },
  exclusionsSectors: { ko: "제외 섹터 수" as UiText },
  exclusionsTypes: { ko: "제외 종목 유형 수" as UiText },
  capsLabel: { ko: "비중 상한" as UiText },
  instrumentCap: { ko: "종목 상한" as UiText },
  instrumentWarning: { ko: "종목 주의" as UiText },
  sectorCap: { ko: "섹터 상한" as UiText },
  orderSwitchTitle: { ko: "주문 켜기" as UiText },
  orderSwitchDescription: {
    ko: "꺼져 있으면 신호 카드만 보이고 주문 버튼은 나타나지 않아요." as UiText,
  },
  orderDenied: {
    ko: "확인 창에서 취소되어 주문 기능이 꺼진 상태로 유지돼요." as UiText,
  },
  orderUnavailable: {
    ko: "지금은 확인 창을 열 수 없어 주문 기능이 꺼진 상태로 유지돼요." as UiText,
  },
  orderChanged: { ko: "주문 설정을 반영했어요." as UiText },
  advancedTitle: { ko: "고급" as UiText },
  advancedAiTitle: { ko: "AI" as UiText },
  advancedAiModels: {
    ko: "확률 추정과 심층 연구는 최상위 모델, 요약과 수집은 빠른 모델을 기본으로 써요." as UiText,
  },
  advancedAiBudgetLabel: { ko: "예약 작업 한도 비율" as UiText },
  advancedAiPrivacy: {
    ko: "채팅과 연구에 쓰인 보유·거래 내용은 AI 제공자 서버에 보관될 수 있어요. 학습 설정은 제공자 계정에서 바꿀 수 있어요." as UiText,
  },
  advancedNewsTitle: { ko: "뉴스·데이터" as UiText },
  advancedNewsBodyRead: {
    ko: "소스별 AI 본문 읽기는 기본으로 꺼져 있어요." as UiText,
  },
  dartKeyLabel: { ko: "DART 선택 키" as UiText },
  fredKeyLabel: { ko: "FRED 선택 키" as UiText },
  optionalKeyConfigured: { ko: "켜짐" as UiText },
  optionalKeyMissing: { ko: "키를 넣으면 켜집니다" as UiText },
  stateOn: { ko: "켜짐" as UiText },
  stateOff: { ko: "꺼짐" as UiText },
  advancedPrinciplesTitle: { ko: "투자 원칙" as UiText },
  concentrationRuleLabel: { ko: "쏠림 경고 기준(유효 종목 수 미만)" as UiText },
  advancedSystemTitle: { ko: "시스템" as UiText },
  autoStartLabel: { ko: "로그인 시 자동 시작" as UiText },
  autoStartHint: {
    ko: "Mac에 로그인하면 창 없이 켜져서 수집과 예약 작업을 이어 가요. 창은 Dock 아이콘으로 열어요." as UiText,
  },
  autoStartFailed: {
    ko: "자동 시작 설정을 바꾸지 못했어요. 잠시 뒤 다시 시도해 주세요." as UiText,
  },
  accountSelectionLabel: { ko: "연결 계좌 수" as UiText },
  accountSelectionHint: {
    ko: "종합매매 계좌 중 첫 계좌를 자동으로 고르며, 계좌가 여럿이면 여기에서 바꿀 수 있어요." as UiText,
  },
  dataToolsHint: {
    ko: "암호화 백업, 내보내기, 전체 삭제는 데이터 저장소가 준비되면 활성화돼요." as UiText,
  },
  openStatusButton: { ko: "상태 보기" as UiText },
  loadingTitle: { ko: "설정을 불러오는 중이에요" as UiText },
  errorTitle: { ko: "설정을 불러오지 못했어요" as UiText },
  retryAction: { ko: "다시 시도" as UiText },
} as const;

export type SettingsMessageKey = keyof typeof catalog;

export const settingsText = (key: SettingsMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
