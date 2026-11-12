declare const test: Readonly<{ fails: (title: "t") => void }>;
export const run = (): void => {
  test.fails("t");
};
