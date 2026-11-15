import { renderCatalog } from "../catalog.mjs";
export default { title: "28 차트/Treemap" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Treemap", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Treemap", "dark"),
};
