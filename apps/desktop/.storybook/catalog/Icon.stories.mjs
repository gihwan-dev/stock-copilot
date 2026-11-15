import { renderCatalog } from "../catalog.mjs";
export default { title: "12 기본/Icon" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Icon", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Icon", "dark"),
};
