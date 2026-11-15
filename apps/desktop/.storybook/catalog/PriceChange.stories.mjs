import { renderCatalog } from "../catalog.mjs";
export default { title: "23 숫자/PriceChange" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("PriceChange", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("PriceChange", "dark"),
};
