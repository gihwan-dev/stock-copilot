declare const it: Readonly<{ skip: (title: "t") => void }>;
const { skip } = it;
export const run = (): void => {
  skip("t");
};
