declare const test: Readonly<{ skip: (title: "t") => void }>;
export const run = (): void => {
  test["skip"]("t");
};
