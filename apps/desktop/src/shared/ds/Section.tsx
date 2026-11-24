import type { ReactElement } from "react";
import type { UiText } from "../lib";
import { Heading } from "./Heading";
import type { LayoutChildren } from "./layout-children";

type Props = LayoutChildren & {
  readonly title: UiText;
};

export const Section = (props: Props): ReactElement => (
  <section className="flex flex-col gap-space-3 w-full max-w-full scroll-flex">
    <Heading level={2}>{props.title}</Heading>
    {props.children}
  </section>
);
