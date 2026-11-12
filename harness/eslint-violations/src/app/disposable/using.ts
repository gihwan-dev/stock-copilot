export const read = (): void => {
  using handle = { [Symbol.dispose]: (): void => undefined };
  handle[Symbol.dispose]();
};
