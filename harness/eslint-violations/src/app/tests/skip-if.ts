declare const test: Readonly<{ skipIf: (flag: "on") => void }>;
export const run = (): void => {
  test.skipIf("on");
};
