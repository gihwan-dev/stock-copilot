import type { ChangeEvent, ReactElement, RefObject } from "react";
export type Slot = { readonly ref: RefObject<HTMLElement | null> };
export const handle = (
  event: Readonly<ChangeEvent<HTMLInputElement>>,
): void => {
  event.preventDefault();
};
export const View = (): ReactElement => <i />;
