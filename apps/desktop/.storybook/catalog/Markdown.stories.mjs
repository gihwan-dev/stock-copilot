import { renderCatalog } from "../catalog.mjs";
export default { title: "29 콘텐츠/Markdown" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Markdown", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Markdown", "dark"),
};
