import { useImperativeHandle } from "react";
export const useHandle = (ref: Readonly<{ current: unknown }>): void => {
  useImperativeHandle(ref, () => "a", []);
};
