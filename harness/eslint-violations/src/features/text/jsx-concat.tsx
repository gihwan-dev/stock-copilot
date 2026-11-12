import type { ReactElement } from "react";
export const Answer = (props: { readonly word: "a" | "b" }): ReactElement => (
  <b>{"OK " + props.word}</b>
);
