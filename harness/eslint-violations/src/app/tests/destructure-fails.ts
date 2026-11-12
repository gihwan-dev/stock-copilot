declare const it: Readonly<{ fails: (title: "t") => void }>;
const { fails } = it;
export const run = (): void => {
  fails("t");
};
