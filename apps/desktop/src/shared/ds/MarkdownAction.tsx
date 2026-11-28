import type { ReactElement } from "react";
import type { ExternalUrl, FreeText } from "../lib";
import type { MarkdownProps } from "./markdown-types";
import type { MarkdownCitation } from "./markdown-citations";
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

export const resolveMarkdownAction = (
  citation: MarkdownCitation | undefined,
  url: ExternalUrl | undefined,
  props:
    | Pick<MarkdownProps, "onLocalCitation" | "linkAllowed" | "onOpenLink">
    | undefined,
): (() => void) | undefined => {
  if (props === undefined) return undefined;
  const open = props.onLocalCitation;
  if (citation?.url === null && open !== undefined)
    return (): void => {
      open(citation.number);
    };
  if (url === undefined || props.linkAllowed(url) === "deny") return undefined;
  return (): void => {
    props.onOpenLink(url);
  };
};
