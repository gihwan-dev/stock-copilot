import { renderCatalog } from "../catalog.mjs";
export default { title: "11 기본/Disclosure" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Disclosure", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Disclosure", "dark"),
};
