import { renderCatalog } from "../catalog.mjs";
export default { title: "숫자/ChartNumber" };
export const Light0 = {
  name: "기본 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("ChartNumber", "light", 0),
};
export const Dark0 = {
  name: "기본 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("ChartNumber", "dark", 0),
};
export const Light1 = {
  name: "부하 · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("ChartNumber", "light", 1),
};
export const Dark1 = {
  name: "부하 · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("ChartNumber", "dark", 1),
};
