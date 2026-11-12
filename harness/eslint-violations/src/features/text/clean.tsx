import type { ReactElement } from "react";
import { homeText } from "../home/messages";
export const Title = (): ReactElement => (
  <b>
    <i key="title">{homeText("home.title")}</i>
  </b>
);
