declare const it: (
  title: "t",
  options: Readonly<{ todo: true }>,
  body: () => void,
) => void;
export const run = (): void => {
  it("t", { todo: true }, () => undefined);
};
