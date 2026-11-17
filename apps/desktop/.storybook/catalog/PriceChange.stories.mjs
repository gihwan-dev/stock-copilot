import { renderCatalog } from "../catalog.mjs";
export default { title: "숫자/PriceChange" };
export const Light0 = {
  name: "기본 · up · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("PriceChange", "light", 0),
};
export const Dark0 = {
  name: "기본 · up · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("PriceChange", "dark", 0),
};
export const Light1 = {
  name: "기본 · down · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("PriceChange", "light", 1),
};
export const Dark1 = {
  name: "기본 · down · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("PriceChange", "dark", 1),
};
export const Light2 = {
  name: "기본 · flat · 003 · 라이트",
  parameters: { theme: "light", sample: 2 },
  render: () => renderCatalog("PriceChange", "light", 2),
};
export const Dark2 = {
  name: "기본 · flat · 003 · 다크",
  parameters: { theme: "dark", sample: 2 },
  render: () => renderCatalog("PriceChange", "dark", 2),
};
export const Light3 = {
  name: "부하 · up · 004 · 라이트",
  parameters: { theme: "light", sample: 3 },
  render: () => renderCatalog("PriceChange", "light", 3),
};
export const Dark3 = {
  name: "부하 · up · 004 · 다크",
  parameters: { theme: "dark", sample: 3 },
  render: () => renderCatalog("PriceChange", "dark", 3),
};
