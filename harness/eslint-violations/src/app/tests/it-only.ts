declare const it: Readonly<{ only: (title: "t") => void }>;
export const run = (): void => {
  it.only("t");
};
