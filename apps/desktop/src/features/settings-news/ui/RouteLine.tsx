import type { ReactElement } from "react";
import { Text } from "../../../shared/ds";
import type { RouteState } from "../../../shared/ipc";
import { newsText } from "../messages";

export const RouteLine = (props: {
  readonly route: "dart" | "sec";
  readonly state: RouteState;
}): ReactElement => (
  <Text
    variant="body-sm"
    tone={props.state === "failing" ? "alert" : "ink-muted"}
  >
    {newsText(`news.route.${props.route}.${props.state}`, "ko")}
  </Text>
);
