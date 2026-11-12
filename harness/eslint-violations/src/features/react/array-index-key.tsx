import type { ReactElement } from "react";
export const List = (
  props: Readonly<{ items: readonly ("a" | "b")[] }>,
): ReactElement => (
  <ul>
    {props.items.map((item, index) => (
      <li key={index}>{item}</li>
    ))}
  </ul>
);
