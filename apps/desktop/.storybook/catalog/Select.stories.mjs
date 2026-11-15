import { renderCatalog } from "../catalog.mjs";
export default { title: "07 기본/Select" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Select", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Select", "dark"),
};
