import { renderCatalog } from "../catalog.mjs";
export default { title: "04 기본/Button" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Button", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Button", "dark"),
};
