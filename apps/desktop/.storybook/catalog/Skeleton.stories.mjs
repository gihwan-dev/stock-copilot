import { renderCatalog } from "../catalog.mjs";
export default { title: "18 피드백/Skeleton" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Skeleton", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Skeleton", "dark"),
};
