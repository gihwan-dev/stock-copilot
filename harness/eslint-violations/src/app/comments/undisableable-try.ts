export const run = (task: () => "a"): "a" | "b" => {
  // eslint-disable-next-line functional/no-try-statements -- 끌 수 없는 규칙
  try {
    return task();
  } finally {
    task();
  }
};
