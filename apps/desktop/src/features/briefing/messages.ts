import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  morning: { ko: "아침 브리핑" as UiText },
  "closing-review": { ko: "마감 리뷰" as UiText },
  "pre-us-market": { ko: "미국장 전 점검" as UiText },
  historyTitle: { ko: "지난 브리핑" as UiText },
  openHistory: { ko: "지난 브리핑 보기" as UiText },
  openBriefing: { ko: "브리핑 보기" as UiText },
  openReader: { ko: "크게 보기" as UiText },
  closeReader: { ko: "닫기" as UiText },
  backToHome: { ko: "현황으로" as UiText },
  finishedAt: { ko: "만든 시각" as UiText },
  sources: { ko: "출처" as UiText },
  observedAt: { ko: "자료 확인 시각" as UiText },
  noSources: { ko: "출처 없음" as UiText },
  sourceWeb: { ko: "미검증 웹 값" as UiText },
  sourceNews: { ko: "뉴스" as UiText },
  sourceDisclosure: { ko: "공시" as UiText },
  noOriginal: { ko: "원문 없음" as UiText },
  openSource: { ko: "원문 열기" as UiText },
  aiNotice: {
    ko: "AI가 모은 자료를 요약한 글이에요. 매매 권유가 아니에요." as UiText,
  },
  failedTitle: { ko: "이번 브리핑은 만들지 못했어요" as UiText },
  "ai-unavailable": {
    ko: "AI를 쓸 수 없어 건너뛰었어요. 다음 예약 시각에 다시 만들어요." as UiText,
  },
  "rate-limited": {
    ko: "AI 사용 한도에 닿아 건너뛰었어요. 한도가 풀리면 다음 예약 시각에 다시 만들어요." as UiText,
  },
  empty: { ko: "AI가 빈 답을 돌려줘 건너뛰었어요." as UiText },
  rejected: { ko: "브리핑 요청을 만들지 못해 건너뛰었어요." as UiText },
  loadingTitle: { ko: "브리핑을 불러오는 중이에요" as UiText },
  emptyTitle: { ko: "아직 브리핑이 없어요" as UiText },
  emptyBody: {
    ko: "아침 8시 20분에 오늘 브리핑이 여기에 보여요." as UiText,
  },
  historyEmptyTitle: { ko: "지난 브리핑이 없어요" as UiText },
  historyEmptyBody: {
    ko: "브리핑이 쌓이면 날짜별로 여기에 남아요." as UiText,
  },
  errorTitle: { ko: "브리핑을 불러오지 못했어요" as UiText },
  retry: { ko: "다시 시도" as UiText },
} as const;

export type BriefingMessageKey = keyof typeof catalog;

export const briefingText = (key: BriefingMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
