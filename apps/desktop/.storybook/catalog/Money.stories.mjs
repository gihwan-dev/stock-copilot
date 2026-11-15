import { renderCatalog } from "../catalog.mjs";
export default { title: "19 숫자/Money" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Money", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Money", "dark"),
};
