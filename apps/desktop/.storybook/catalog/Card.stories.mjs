import { renderCatalog } from "../catalog.mjs";
export default { title: "01 레이아웃/Card" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Card", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Card", "dark"),
};
