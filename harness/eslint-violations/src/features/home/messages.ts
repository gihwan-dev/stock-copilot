import type { UiText } from "../../shared/lib/brand";
const catalog = { "home.title": { ko: "내 현황" } } as const;
export const homeText = (key: keyof typeof catalog): UiText =>
  catalog[key].ko as UiText;
