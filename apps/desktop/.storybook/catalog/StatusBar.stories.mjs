import { renderCatalog } from "../catalog.mjs";
export default { title: "15 피드백/StatusBar" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("StatusBar", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("StatusBar", "dark"),
};
