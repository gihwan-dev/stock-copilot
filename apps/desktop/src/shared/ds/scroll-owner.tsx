import { createContext, use, type ReactElement } from "react";

type ScrollOwner = "own" | "card";

const ScrollOwnerContext = createContext<ScrollOwner>("own");

export const ScrollOwnerScope = (props: {
  readonly owner: ScrollOwner;
  readonly children: ReactElement;
}): ReactElement => (
  <ScrollOwnerContext value={props.owner}>{props.children}</ScrollOwnerContext>
);

export const useScrollOwner = (): ScrollOwner => use(ScrollOwnerContext);
