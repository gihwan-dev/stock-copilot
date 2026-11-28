import type { UiText } from "../../shared/lib";

export const researchText = {
  detail: "종목 상세" as UiText,
  asOf: "분석 기준" as UiText,
  open: "원문 열기" as UiText,
  holdings: "보유 자산 평가" as UiText,
  candidates: "새 종목 후보" as UiText,
  economy: "경제와 산업 동향" as UiText,
  empty: "아직 조사 결과가 없어요." as UiText,
  refreshing: "새 자료를 확인하고 있어요." as UiText,
  failed: "이번 조사 결과를 불러오지 못했어요." as UiText,
  countUnit: "건" as UiText,
  sources: "근거 자료" as UiText,
  loading: "조사 결과를 불러오고 있어요." as UiText,
} as const;
