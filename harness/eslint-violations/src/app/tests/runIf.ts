declare const test: Readonly<{ runIf: (title: "t") => void }>;
export const run = (): void => {
  test.runIf("t");
};
