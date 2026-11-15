import { renderCatalog } from "../catalog.mjs";
export default { title: "13 피드백/Table" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Table", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Table", "dark"),
};
