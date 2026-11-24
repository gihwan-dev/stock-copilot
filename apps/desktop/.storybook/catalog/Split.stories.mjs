import { renderCatalog } from "../catalog.mjs";
export default { title: "레이아웃/Split" };
export const Light0 = {
  name: "기본 · sidebar-width · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("Split", "light", 0),
};
export const Dark0 = {
  name: "기본 · sidebar-width · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("Split", "dark", 0),
};
export const Light1 = {
  name: "기본 · sidebar-collapsed · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("Split", "light", 1),
};
export const Dark1 = {
  name: "기본 · sidebar-collapsed · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("Split", "dark", 1),
};
export const Light2 = {
  name: "기본 · chat-width · 003 · 라이트",
  parameters: { theme: "light", sample: 2 },
  render: () => renderCatalog("Split", "light", 2),
};
export const Dark2 = {
  name: "기본 · chat-width · 003 · 다크",
  parameters: { theme: "dark", sample: 2 },
  render: () => renderCatalog("Split", "dark", 2),
};
export const Light3 = {
  name: "기본 · chat-width-min · 004 · 라이트",
  parameters: { theme: "light", sample: 3 },
  render: () => renderCatalog("Split", "light", 3),
};
export const Dark3 = {
  name: "기본 · chat-width-min · 004 · 다크",
  parameters: { theme: "dark", sample: 3 },
  render: () => renderCatalog("Split", "dark", 3),
};
export const Light4 = {
  name: "긴 내용 · sidebar-width · 005 · 라이트",
  parameters: { theme: "light", sample: 4 },
  render: () => renderCatalog("Split", "light", 4),
};
export const Dark4 = {
  name: "긴 내용 · sidebar-width · 005 · 다크",
  parameters: { theme: "dark", sample: 4 },
  render: () => renderCatalog("Split", "dark", 4),
};
export const Light5 = {
  name: "부하 · sidebar-width · 006 · 라이트",
  parameters: { theme: "light", sample: 5 },
  render: () => renderCatalog("Split", "light", 5),
};
export const Dark5 = {
  name: "부하 · sidebar-width · 006 · 다크",
  parameters: { theme: "dark", sample: 5 },
  render: () => renderCatalog("Split", "dark", 5),
};
