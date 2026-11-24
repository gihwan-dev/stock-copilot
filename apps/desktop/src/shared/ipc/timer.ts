const ipcTimeoutMs = 10_000;

export const startIpcTimeout = (onTimeout: () => void): (() => void) => {
  const handle = setTimeout(onTimeout, ipcTimeoutMs);
  return (): void => {
    clearTimeout(handle);
  };
};
