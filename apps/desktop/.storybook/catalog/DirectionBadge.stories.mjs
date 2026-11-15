import { renderCatalog } from "../catalog.mjs";
export default { title: "24 숫자/DirectionBadge" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("DirectionBadge", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("DirectionBadge", "dark"),
};
