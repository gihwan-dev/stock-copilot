export const read = async (): Promise<void> => {
  await using handle = {
    [Symbol.asyncDispose]: async (): Promise<void> => undefined,
  };
  await handle[Symbol.asyncDispose]();
};
