declare const it: (
  title: "t",
  options: Readonly<{ skip: true }>,
  body: () => void,
) => void;
export const run = (): void => {
  it("t", { skip: true }, () => undefined);
};
