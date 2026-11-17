import type { UiText } from "../../lib";

const catalog = {
  company: {
    ko: "대한미래첨단산업글로벌에너지반도체투자지주회사대한미래첨단산업주식회사기술금융원" as UiText,
  },
  english: {
    ko: "ExtraordinarilyLongUnbrokenInternationalSemiconductorInvestmentCompanyName" as UiText,
  },
  paragraph: {
    ko: "실적과 현금 흐름을 살펴보고 여러 출처를 비교한 가상의 근거 문단입니다." as UiText,
  },
  empty: { ko: "" as UiText },
} as const;

export type StressMessage = keyof typeof catalog;
export const stressMessage = (key: StressMessage, locale: "ko"): UiText =>
  catalog[key][locale];
