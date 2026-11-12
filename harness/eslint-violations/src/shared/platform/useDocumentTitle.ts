import { useSyncExternalStore } from "react";
const subscribe = (): (() => void) => (): void => undefined;
const snapshot = (): "title" => "title";
export const useDocumentTitle = (): "title" =>
  useSyncExternalStore(subscribe, snapshot);
