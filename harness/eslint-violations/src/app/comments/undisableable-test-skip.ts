export const run = (suite: Readonly<{ skip: "x" }>): unknown =>
  // eslint-disable-next-line guard/no-test-skip -- 끌 수 없는 규칙
  suite.skip;
