import { renderCatalog } from "../catalog.mjs";
export default { title: "레이아웃/AppShell" };
export const Light0 = {
  name: "기본 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("AppShell", "light", 0),
};
export const Dark0 = {
  name: "기본 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("AppShell", "dark", 0),
};
export const Light1 = {
  name: "열림 · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("AppShell", "light", 1),
};
export const Dark1 = {
  name: "열림 · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("AppShell", "dark", 1),
};
export const Light2 = {
  name: "긴 내용 · 003 · 라이트",
  parameters: { theme: "light", sample: 2 },
  render: () => renderCatalog("AppShell", "light", 2),
};
export const Dark2 = {
  name: "긴 내용 · 003 · 다크",
  parameters: { theme: "dark", sample: 2 },
  render: () => renderCatalog("AppShell", "dark", 2),
};
export const Light3 = {
  name: "부하 · 004 · 라이트",
  parameters: { theme: "light", sample: 3 },
  render: () => renderCatalog("AppShell", "light", 3),
};
export const Dark3 = {
  name: "부하 · 004 · 다크",
  parameters: { theme: "dark", sample: 3 },
  render: () => renderCatalog("AppShell", "dark", 3),
};
