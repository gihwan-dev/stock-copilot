import type { UiText } from "../../shared/lib/brand";
const word = "제목";
const catalog = { "a.title": { ko: word as UiText } } as const;
export const title = (key: "a.title"): UiText => catalog[key].ko;
