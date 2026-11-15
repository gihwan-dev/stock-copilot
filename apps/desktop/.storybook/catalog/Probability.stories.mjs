import { renderCatalog } from "../catalog.mjs";
export default { title: "22 숫자/Probability" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Probability", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Probability", "dark"),
};
