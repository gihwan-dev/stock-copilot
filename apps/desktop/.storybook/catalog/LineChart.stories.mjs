import { renderCatalog } from "../catalog.mjs";
export default { title: "25 차트/LineChart" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("LineChart", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("LineChart", "dark"),
};
