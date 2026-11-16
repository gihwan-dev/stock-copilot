import { renderCatalog } from "../catalog.mjs";
export default { title: "기본/Switch" };
export const Light0 = {
  name: "기본 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("Switch", "light", 0),
};
export const Dark0 = {
  name: "기본 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("Switch", "dark", 0),
};
export const Light1 = {
  name: "비활성 · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("Switch", "light", 1),
};
export const Dark1 = {
  name: "비활성 · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("Switch", "dark", 1),
};
export const Light2 = {
  name: "기본 · 003 · 라이트",
  parameters: { theme: "light", sample: 2 },
  render: () => renderCatalog("Switch", "light", 2),
};
export const Dark2 = {
  name: "기본 · 003 · 다크",
  parameters: { theme: "dark", sample: 2 },
  render: () => renderCatalog("Switch", "dark", 2),
};
export const Light3 = {
  name: "비활성 · 004 · 라이트",
  parameters: { theme: "light", sample: 3 },
  render: () => renderCatalog("Switch", "light", 3),
};
export const Dark3 = {
  name: "비활성 · 004 · 다크",
  parameters: { theme: "dark", sample: 3 },
  render: () => renderCatalog("Switch", "dark", 3),
};
