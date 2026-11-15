import { renderCatalog } from "../catalog.mjs";
export default { title: "06 기본/TextField" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("TextField", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("TextField", "dark"),
};
