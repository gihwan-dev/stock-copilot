declare const test: Readonly<{ concurrent: (title: "t") => void }>;
export const run = (): void => {
  test.concurrent("t");
};
