import { renderCatalog } from "../catalog.mjs";
export default { title: "콘텐츠/ConversationTree" };
export const Light0 = {
  name: "기본 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("ConversationTree", "light", 0),
};
export const Dark0 = {
  name: "기본 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("ConversationTree", "dark", 0),
};
export const Light1 = {
  name: "부하 · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("ConversationTree", "light", 1),
};
export const Dark1 = {
  name: "부하 · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("ConversationTree", "dark", 1),
};
