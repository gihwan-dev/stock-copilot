import type { ReactElement } from "react";
import { greetingText } from "../messages";
export const Title = (): ReactElement => (
  <b>
    <i key="title">{greetingText("greeting.title", "ko")}</i>
  </b>
);
