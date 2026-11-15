import { renderCatalog } from "../catalog.mjs";
export default { title: "14 피드백/Badge" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Badge", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Badge", "dark"),
};
