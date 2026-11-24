import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  "ai.title": { ko: "AI" as UiText },
  "ai.loading": { ko: "불러오는 중이에요" as UiText },
  "ai.loadFailed": {
    ko: "AI 설정을 불러오지 못했어요. 잠시 뒤 다시 열어 주세요." as UiText,
  },
  "ai.saveFailed": {
    ko: "저장하지 못했어요. 다시 시도해 주세요." as UiText,
  },
  "ai.saved": { ko: "저장했어요" as UiText },
  "ai.modelsTitle": { ko: "작업별 모델" as UiText },
  "ai.modelsBody": {
    ko: "모델을 바꿔도 대화 기록, 논거 노트, 추정 기록, 성적표는 그대로예요." as UiText,
  },
  "ai.taskHeader": { ko: "작업" as UiText },
  "ai.modelHeader": { ko: "모델" as UiText },
  "ai.effortHeader": { ko: "추론 강도" as UiText },
  "ai.task.chat": { ko: "채팅" as UiText },
  "ai.task.estimate": { ko: "확률 추정" as UiText },
  "ai.task.deep-research": { ko: "심층 연구" as UiText },
  "ai.task.briefing": { ko: "브리핑과 리뷰" as UiText },
  "ai.task.news-summary": { ko: "뉴스 분류와 요약" as UiText },
  "ai.task.collection": { ko: "웹 수집" as UiText },
  "ai.model.opus": { ko: "Claude Opus" as UiText },
  "ai.model.sonnet": { ko: "Claude Sonnet" as UiText },
  "ai.model.haiku": { ko: "Claude Haiku" as UiText },
  "ai.effort.low": { ko: "낮음" as UiText },
  "ai.effort.medium": { ko: "보통" as UiText },
  "ai.effort.high": { ko: "높음" as UiText },
  "ai.effort.xhigh": { ko: "매우 높음" as UiText },
  "ai.effort.max": { ko: "최대" as UiText },
  "ai.budgetTitle": { ko: "예약 작업 한도" as UiText },
  "ai.budgetLabel": { ko: "예약 작업이 쓸 수 있는 구독 한도" as UiText },
  "ai.budgetHint": {
    ko: "채팅은 늘 먼저예요. 예약 작업은 이 비율에 닿으면 쉬었다가 이어 가요." as UiText,
  },
  "ai.budget.50": { ko: "한도의 50%" as UiText },
  "ai.budget.60": { ko: "한도의 60%" as UiText },
  "ai.budget.70": { ko: "한도의 70%" as UiText },
  "ai.budget.80": { ko: "한도의 80%" as UiText },
  "ai.budget.90": { ko: "한도의 90%" as UiText },
  "ai.usageLabel": { ko: "지금까지 쓴 한도" as UiText },
  "ai.usageUnknown": { ko: "사용량을 아직 알 수 없어요" as UiText },
  "ai.configTitle": { ko: "Claude 설정 위치" as UiText },
  "ai.configLabel": { ko: "설정 폴더" as UiText },
  "ai.configHint": {
    ko: "기본 위치를 쓰면 비워 두세요. 다른 폴더를 쓸 때만 적어요." as UiText,
  },
  "ai.configInvalid": {
    ko: "/ 또는 ~로 시작하는 폴더 경로를 적어 주세요." as UiText,
  },
  "ai.save": { ko: "저장" as UiText },
  "ai.privacyTitle": { ko: "데이터 전송 안내" as UiText },
  "ai.privacyBody": {
    ko: "AI에 보낸 보유 종목, 비중, 손익, 거래내역, 노트는 Anthropic 서버에 보관돼요. 보관 기간은 Claude 계정의 학습 설정에 따라 달라요." as UiText,
  },
  "ai.privacyOpen": { ko: "Anthropic 학습 설정 열기" as UiText },
  "ai.privacyFailed": {
    ko: "설정 페이지를 열지 못했어요." as UiText,
  },
  "ai.jobsTitle": { ko: "예약 작업" as UiText },
  "ai.jobsBody": {
    ko: "앱이 켜져 있는 동안 정해진 시각에 돌아요. 실행 기록은 고급 > 상태에서 볼 수 있어요." as UiText,
  },
  "ai.job.morning-briefing": { ko: "아침 브리핑" as UiText },
  "ai.job.market-watch": { ko: "장중 감시" as UiText },
  "ai.job.closing-review": { ko: "마감 리뷰" as UiText },
  "ai.job.us-premarket": { ko: "미국장 전 점검" as UiText },
  "ai.job.estimate-rotation": { ko: "종목 추정 순환" as UiText },
  "ai.job.discovery": { ko: "유망 종목 발굴" as UiText },
  "ai.job.weekly-review": { ko: "주간 점검" as UiText },
  "ai.jobTime": { ko: "시각" as UiText },
  "ai.jobTimeInvalid": {
    ko: "08:20처럼 24시간 형식으로 적어 주세요." as UiText,
  },
  "ai.jobEvent": {
    ko: "정해진 시각 없이 일이 생기거나 한도가 남을 때 돌아요." as UiText,
  },
} as const;

export type AiMessageKey = keyof typeof catalog;

export const aiText = (key: AiMessageKey, locale: Locale): UiText =>
  catalog[key][locale];
