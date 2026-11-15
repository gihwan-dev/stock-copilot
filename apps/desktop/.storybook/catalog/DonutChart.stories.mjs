import { renderCatalog } from "../catalog.mjs";
export default { title: "27 차트/DonutChart" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("DonutChart", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("DonutChart", "dark"),
};
