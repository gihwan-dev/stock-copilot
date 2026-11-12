export const load = (): unknown =>
  // eslint-disable-next-line guard/no-network -- 끌 수 없는 규칙
  fetch("https://example.com");
