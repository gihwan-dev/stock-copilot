import { renderCatalog } from "../catalog.mjs";
export default { title: "05 기본/IconButton" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("IconButton", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("IconButton", "dark"),
};
