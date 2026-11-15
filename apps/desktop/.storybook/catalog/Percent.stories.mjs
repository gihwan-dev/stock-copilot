import { renderCatalog } from "../catalog.mjs";
export default { title: "20 숫자/Percent" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Percent", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Percent", "dark"),
};
