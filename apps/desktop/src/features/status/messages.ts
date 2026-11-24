import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  title: { ko: "상태" as UiText },
  backToSettings: { ko: "설정으로" as UiText },
  intro: {
    ko: "앱이 스스로 점검한 기록이에요. 여기 있는 항목은 사용자가 따로 할 일이 없어요." as UiText,
  },
  aiSection: { ko: "AI" as UiText },
  runtimeTitle: { ko: "AI 실행 환경" as UiText },
  runtimeReady: {
    ko: "사전 점검을 통과해 AI 기능을 쓰고 있어요." as UiText,
  },
  runtimeUnavailable: {
    ko: "사전 점검을 통과하지 못해 AI 기능만 꺼 두었어요. 사유:" as UiText,
  },
  limitTitle: { ko: "AI 사용 한도" as UiText },
  limitAvailable: { ko: "한도 안에서 쓰고 있어요." as UiText },
  limitWaiting: {
    ko: "구독 한도에 닿아 예약 작업이 기다리는 중이에요. 재개 시각:" as UiText,
  },
  usageTitle: { ko: "작업별 AI 사용량" as UiText },
  usageTask: { ko: "작업" as UiText },
  usageTokens: { ko: "토큰" as UiText },
  usageSeconds: { ko: "소요 시간(초)" as UiText },
  usageEmpty: { ko: "아직 기록된 AI 작업이 없어요" as UiText },
  sourcesSection: { ko: "수집 소스" as UiText },
  sourcesTitle: { ko: "소스별 마지막 성공 시각" as UiText },
  sourceName: { ko: "소스" as UiText },
  sourceLastSuccess: { ko: "마지막 성공" as UiText },
  sourceNever: { ko: "아직 성공 기록 없음" as UiText },
  sourcesEmpty: { ko: "등록된 수집 소스가 없어요" as UiText },
  recordsSection: { ko: "점검 기록" as UiText },
  recordsTitle: { ko: "최근 점검 기록과 자세한 사유" as UiText },
  recordAt: { ko: "시각" as UiText },
  recordCheck: { ko: "점검" as UiText },
  recordDetail: { ko: "사유" as UiText },
  recordsEmpty: { ko: "아직 점검 기록이 없어요" as UiText },
  loadingTitle: { ko: "상태를 불러오는 중이에요" as UiText },
  emptyTitle: { ko: "아직 점검 기록이 없어요" as UiText },
  emptyBody: {
    ko: "앱이 첫 점검을 마치면 여기에 기록이 쌓여요." as UiText,
  },
  errorTitle: { ko: "상태를 불러오지 못했어요" as UiText },
  retry: { ko: "다시 시도" as UiText },
} as const;

export type StatusMessageKey = keyof typeof catalog;

export const statusText = (key: StatusMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
