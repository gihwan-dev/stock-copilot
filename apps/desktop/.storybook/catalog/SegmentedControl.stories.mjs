import { renderCatalog } from "../catalog.mjs";
export default { title: "10 기본/SegmentedControl" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("SegmentedControl", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("SegmentedControl", "dark"),
};
