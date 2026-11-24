import type { Locale, UiText } from "../../shared/lib";

const catalog = {
  "scorecard.title": { ko: "성적표" as UiText },
  "scorecard.loading": { ko: "성적표를 불러오는 중이에요" as UiText },
  "scorecard.empty": { ko: "아직 판정된 추정이 없어요" as UiText },
  "scorecard.error": { ko: "성적표를 불러오지 못했어요" as UiText },
  "scorecard.retry": { ko: "다시 시도" as UiText },
  "scorecard.combination": { ko: "모델·기간" as UiText },
  "scorecard.judged": { ko: "판정 건수" as UiText },
  "scorecard.pending": {
    ko: "판정 보류: 판정이 30건이 되기 전에는 결과를 단정하지 않아요." as UiText,
  },
  "scorecard.brier": { ko: "Brier 점수" as UiText },
  "scorecard.reliability": { ko: "신뢰도 도표 (구간별 실제 적중률)" as UiText },
  "scorecard.reliabilityEmpty": { ko: "도표로 그릴 구간이 없어요" as UiText },
  "scorecard.binsCaption": { ko: "확률 구간별 적중률과 건수" as UiText },
  "scorecard.bin": { ko: "확률 구간" as UiText },
  "scorecard.meanPrediction": { ko: "평균 예상" as UiText },
  "scorecard.hitRate": { ko: "실제 적중률" as UiText },
  "scorecard.count": { ko: "건수" as UiText },
  "scorecard.signalReturn": {
    ko: "신호를 따랐을 때의 수익률 (가상 청산)" as UiText,
  },
  "scorecard.benchmark": { ko: "벤치마크 수익률" as UiText },
  "scorecard.interval": { ko: "신뢰구간" as UiText },
  "scorecard.intervalTo": { ko: "~" as UiText },
  "scorecard.comparisons": { ko: "비교한 모델·프롬프트 수" as UiText },
  "scorecard.notYet": { ko: "아직 계산되지 않았어요" as UiText },
} as const;

export type ScorecardMessageKey = keyof typeof catalog;

export const scorecardText = (
  key: ScorecardMessageKey,
  locale: Locale,
): UiText => catalog[key][locale];
