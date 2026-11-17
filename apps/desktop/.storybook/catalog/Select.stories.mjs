import { renderCatalog } from "../catalog.mjs";
export default { title: "기본/Select" };
export const Light0 = {
  name: "기본 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("Select", "light", 0),
};
export const Dark0 = {
  name: "기본 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("Select", "dark", 0),
};
export const Light1 = {
  name: "도움말 · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("Select", "light", 1),
};
export const Dark1 = {
  name: "도움말 · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("Select", "dark", 1),
};
export const Light2 = {
  name: "오류 · 003 · 라이트",
  parameters: { theme: "light", sample: 2 },
  render: () => renderCatalog("Select", "light", 2),
};
export const Dark2 = {
  name: "오류 · 003 · 다크",
  parameters: { theme: "dark", sample: 2 },
  render: () => renderCatalog("Select", "dark", 2),
};
export const Light3 = {
  name: "비활성 · 004 · 라이트",
  parameters: { theme: "light", sample: 3 },
  render: () => renderCatalog("Select", "light", 3),
};
export const Dark3 = {
  name: "비활성 · 004 · 다크",
  parameters: { theme: "dark", sample: 3 },
  render: () => renderCatalog("Select", "dark", 3),
};
export const Light4 = {
  name: "부하 · 005 · 라이트",
  parameters: { theme: "light", sample: 4 },
  render: () => renderCatalog("Select", "light", 4),
};
export const Dark4 = {
  name: "부하 · 005 · 다크",
  parameters: { theme: "dark", sample: 4 },
  render: () => renderCatalog("Select", "dark", 4),
};
