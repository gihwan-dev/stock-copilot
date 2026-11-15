import { renderCatalog } from "../catalog.mjs";
export default { title: "03 기본/Heading" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Heading", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Heading", "dark"),
};
