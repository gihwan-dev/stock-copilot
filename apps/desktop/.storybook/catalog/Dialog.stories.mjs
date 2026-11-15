import { renderCatalog } from "../catalog.mjs";
export default { title: "16 피드백/Dialog" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("Dialog", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("Dialog", "dark"),
};
