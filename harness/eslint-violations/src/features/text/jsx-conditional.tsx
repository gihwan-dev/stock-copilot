import type { ReactElement } from "react";
export const Answer = (props: {
  readonly state: "on" | "off";
}): ReactElement => <b>{props.state === "on" ? "Yes" : "No"}</b>;
