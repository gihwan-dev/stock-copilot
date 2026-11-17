import { renderCatalog } from "../catalog.mjs";
export default { title: "콘텐츠/ChatInput" };
export const Light0 = {
  name: "기본 · 001 · 라이트",
  parameters: { theme: "light", sample: 0 },
  render: () => renderCatalog("ChatInput", "light", 0),
};
export const Dark0 = {
  name: "기본 · 001 · 다크",
  parameters: { theme: "dark", sample: 0 },
  render: () => renderCatalog("ChatInput", "dark", 0),
};
export const Light1 = {
  name: "전송 중 · 002 · 라이트",
  parameters: { theme: "light", sample: 1 },
  render: () => renderCatalog("ChatInput", "light", 1),
};
export const Dark1 = {
  name: "전송 중 · 002 · 다크",
  parameters: { theme: "dark", sample: 1 },
  render: () => renderCatalog("ChatInput", "dark", 1),
};
export const Light2 = {
  name: "부하 · 003 · 라이트",
  parameters: { theme: "light", sample: 2 },
  render: () => renderCatalog("ChatInput", "light", 2),
};
export const Dark2 = {
  name: "부하 · 003 · 다크",
  parameters: { theme: "dark", sample: 2 },
  render: () => renderCatalog("ChatInput", "dark", 2),
};
export const Light3 = {
  name: "부하 · 004 · 라이트",
  parameters: { theme: "light", sample: 3 },
  render: () => renderCatalog("ChatInput", "light", 3),
};
export const Dark3 = {
  name: "부하 · 004 · 다크",
  parameters: { theme: "dark", sample: 3 },
  render: () => renderCatalog("ChatInput", "dark", 3),
};
