import type { ReactElement } from "react";
import type { FreeText } from "../lib";
import { CitationTooltip } from "./Tooltip";

export const MarkdownAction = (props: {
  readonly children: ReactElement;
  readonly onOpen: () => void;
  readonly citation?: FreeText | undefined;
}): ReactElement => {
  const button = (
    <button
      type="button"
      role={props.citation === undefined ? undefined : "link"}
      className="inline-block markdown-link text-left text-primary focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
      onClick={props.onOpen}
    >
      {props.children}
    </button>
  );
  return props.citation === undefined ? (
    button
  ) : (
    <CitationTooltip detail={props.citation} trigger={button} />
  );
};
