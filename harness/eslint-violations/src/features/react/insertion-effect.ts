import { useInsertionEffect } from "react";
export const useRun = (): void => {
  useInsertionEffect(() => undefined, []);
};
