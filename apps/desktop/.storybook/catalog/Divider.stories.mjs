import { renderCatalog } from "../catalog.mjs";
export default { title: "레이아웃/Divider" };
export const Light0 = {
  name: "기본 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("Divider", "light", 0),
};
export const Dark0 = {
  name: "기본 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("Divider", "dark", 0),
};
export const Light1 = {
  name: "기본 · vertical · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("Divider", "light", 1),
};
export const Dark1 = {
  name: "기본 · vertical · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("Divider", "dark", 1),
};
export const Light2 = {
  name: "부하 · 003 · 라이트",
  parameters: { theme: "light", sample: 2 },
  render: () => renderCatalog("Divider", "light", 2),
};
export const Dark2 = {
  name: "부하 · 003 · 다크",
  parameters: { theme: "dark", sample: 2 },
  render: () => renderCatalog("Divider", "dark", 2),
};
