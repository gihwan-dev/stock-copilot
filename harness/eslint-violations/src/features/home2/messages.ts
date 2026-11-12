import type { EntityId, UiText } from "../../shared/lib/brand";
const catalog = { "a.title": { ko: "제목" as UiText } } as const;
export const fromId = (id: EntityId): UiText =>
  id === "x" ? catalog["a.title"].ko : catalog["a.title"].ko;
