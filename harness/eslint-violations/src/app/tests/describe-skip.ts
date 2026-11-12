declare const describe: Readonly<{ skip: (title: "t") => void }>;
export const run = (): void => {
  describe.skip("t");
};
