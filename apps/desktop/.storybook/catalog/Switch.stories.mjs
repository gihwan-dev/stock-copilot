import { renderCatalog } from "../catalog.mjs";
export default { title: "09 기본/Switch" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Switch", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Switch", "dark"),
};
