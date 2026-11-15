import { renderCatalog } from "../catalog.mjs";
export default { title: "21 숫자/Quantity" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Quantity", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Quantity", "dark"),
};
