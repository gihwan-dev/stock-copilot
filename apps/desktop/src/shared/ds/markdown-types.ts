import type { EntityId, ExternalUrl, FreeText } from "../lib";
import type { MarkdownCitation } from "./markdown-citations";

export type MarkdownProps = {
  readonly content: FreeText;
  readonly linkAllowed: (url: ExternalUrl) => "allow" | "deny";
  readonly onOpenLink: (url: ExternalUrl) => void;
  readonly onLocalCitation?: (number: EntityId) => void;
  readonly citations?: readonly MarkdownCitation[];
  readonly scale?: "body" | "reading" | "lead";
  readonly flavor?: "basic" | "gfm";
  readonly appearance?: "plain" | "question" | "briefing" | "research";
};
