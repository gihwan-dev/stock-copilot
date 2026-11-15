import { renderCatalog } from "../catalog.mjs";
export default { title: "기본/Disclosure" };
export const Light0 = {
  name: "닫힘 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("Disclosure", "light", 0),
};
export const Dark0 = {
  name: "닫힘 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("Disclosure", "dark", 0),
};
export const Light1 = {
  name: "열림 · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("Disclosure", "light", 1),
};
export const Dark1 = {
  name: "열림 · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("Disclosure", "dark", 1),
};
