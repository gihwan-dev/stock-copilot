import { useSyncExternalStore } from "react";
export const useValue = (): unknown =>
  useSyncExternalStore(
    () => (): void => undefined,
    () => "a",
  );
