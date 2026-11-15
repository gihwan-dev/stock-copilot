import { renderCatalog } from "../catalog.mjs";
export default { title: "02 기본/Text" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Text", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Text", "dark"),
};
