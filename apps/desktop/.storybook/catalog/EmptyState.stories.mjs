import { renderCatalog } from "../catalog.mjs";
export default { title: "17 피드백/EmptyState" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("EmptyState", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("EmptyState", "dark"),
};
