export const fail = (): never => {
  // eslint-disable-next-line functional/no-throw-statements -- 끌 수 없는 규칙
  throw new Error("x");
};
