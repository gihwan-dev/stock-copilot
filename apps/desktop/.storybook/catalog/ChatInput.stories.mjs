import { renderCatalog } from "../catalog.mjs";
export default { title: "30 콘텐츠/ChatInput" };
export const Light = {
  name: "라이트",
  parameters: { theme: "light" },
  render: () => renderCatalog("ChatInput", "light"),
};
export const Dark = {
  name: "다크",
  parameters: { theme: "dark" },
  render: () => renderCatalog("ChatInput", "dark"),
};
