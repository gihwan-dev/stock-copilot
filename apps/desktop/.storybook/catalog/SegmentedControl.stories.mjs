import { renderCatalog } from "../catalog.mjs";
export default { title: "기본/SegmentedControl" };
export const Light0 = {
  name: "선택 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("SegmentedControl", "light", 0),
};
export const Dark0 = {
  name: "선택 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("SegmentedControl", "dark", 0),
};
export const Light1 = {
  name: "선택 · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("SegmentedControl", "light", 1),
};
export const Dark1 = {
  name: "선택 · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("SegmentedControl", "dark", 1),
};
export const Light2 = {
  name: "선택 · 003 · 라이트",
  parameters: { theme: "light", sample: 2 },
  render: () => renderCatalog("SegmentedControl", "light", 2),
};
export const Dark2 = {
  name: "선택 · 003 · 다크",
  parameters: { theme: "dark", sample: 2 },
  render: () => renderCatalog("SegmentedControl", "dark", 2),
};
