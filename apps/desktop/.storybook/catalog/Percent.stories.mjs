import { renderCatalog } from "../catalog.mjs";
export default { title: "숫자/Percent" };
export const Light0 = {
  name: "기본 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("Percent", "light", 0),
};
export const Dark0 = {
  name: "기본 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("Percent", "dark", 0),
};
export const Light1 = {
  name: "기본 · up · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("Percent", "light", 1),
};
export const Dark1 = {
  name: "기본 · up · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("Percent", "dark", 1),
};
export const Light2 = {
  name: "기본 · down · 003 · 라이트",
  parameters: { theme: "light", sample: 2 },
  render: () => renderCatalog("Percent", "light", 2),
};
export const Dark2 = {
  name: "기본 · down · 003 · 다크",
  parameters: { theme: "dark", sample: 2 },
  render: () => renderCatalog("Percent", "dark", 2),
};
export const Light3 = {
  name: "기본 · flat · 004 · 라이트",
  parameters: { theme: "light", sample: 3 },
  render: () => renderCatalog("Percent", "light", 3),
};
export const Dark3 = {
  name: "기본 · flat · 004 · 다크",
  parameters: { theme: "dark", sample: 3 },
  render: () => renderCatalog("Percent", "dark", 3),
};
