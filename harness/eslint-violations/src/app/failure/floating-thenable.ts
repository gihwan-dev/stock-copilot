type Thenable = Readonly<{
  then: (onDone: () => void, onFail: () => void) => void;
}>;
const start = (): Thenable => ({ then: (): void => undefined });
export const run = (): "b" => {
  start();
  return "b";
};
