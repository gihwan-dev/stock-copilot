export const run = (task: () => "a"): void => {
  // eslint-disable-next-line functional/no-expression-statements -- 끌 수 없는 규칙
  task();
};
