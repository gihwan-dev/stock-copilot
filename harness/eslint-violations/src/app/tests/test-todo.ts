declare const test: Readonly<{ todo: (title: "t") => void }>;
export const run = (): void => {
  test.todo("t");
};
