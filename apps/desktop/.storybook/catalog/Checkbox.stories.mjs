import { renderCatalog } from "../catalog.mjs";
export default { title: "08 기본/Checkbox" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Checkbox", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Checkbox", "dark"),
};
