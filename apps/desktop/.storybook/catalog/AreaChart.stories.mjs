import { renderCatalog } from "../catalog.mjs";
export default { title: "26 차트/AreaChart" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("AreaChart", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("AreaChart", "dark"),
};
