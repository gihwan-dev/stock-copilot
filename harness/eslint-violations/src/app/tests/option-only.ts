declare const it: (
  title: "t",
  options: Readonly<{ only: true }>,
  body: () => void,
) => void;
export const run = (): void => {
  it("t", { only: true }, () => undefined);
};
