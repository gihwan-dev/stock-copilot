import type { MarkdownProps } from "./markdown-types";
import type { MarkdownParent as Parent } from "./Markdown";
import type { EntityId, ExternalUrl, FreeText } from "../lib";

export type MarkdownCitation = {
  readonly number: EntityId;
  readonly url: ExternalUrl | null;
  readonly title: FreeText;
};
type Node = Readonly<Parent | Parent["children"][0]>;
type Root = Extract<Parent, { type: "root" }>;
type Text = Extract<Node, { type: "text" }>;
type Group = Extract<Node, { type: "emphasis" }>;
type Link = Extract<Node, { type: "link" }>;
type Branch = {
  readonly type: Parent["type"];
  readonly children: readonly (Node | Branch)[];
};

const expandText = (
  node: Text,
  citations: readonly MarkdownCitation[],
): readonly (Text | Link | Group)[] =>
  node.value
    .split(/(\[[1-9]\d*\][.,!?。！？…]*)/u)
    .filter((value) => value.length > 0)
    .map((value): Text | Link | Group => {
      const boundary = value.indexOf("]") + 1;
      const label = value.slice(0, boundary);
      const ref = citations.find((entry) => `[${entry.number}]` === label);
      if (ref === undefined) return { type: "text", value };
      const link: Link = {
        type: "link",
        url: ref.url ?? "",
        children: [{ type: "text", value: label }],
        data: { hProperties: { dataCitation: ref.number } },
      };
      const punctuation = value.slice(boundary);
      if (punctuation.length === 0) return link;
      return {
        type: "emphasis",
        data: {
          hName: "span",
          hProperties: { dataCitationGroup: "joined" },
        },
        children: [link, { type: "text", value: punctuation }],
      };
    });

export const citationPlugin = (
  citations: readonly MarkdownCitation[],
  appearance?: MarkdownProps["appearance"],
): (() => (root: Root) => Branch) => {
  const expand = (node: Node): readonly (Node | Branch)[] => {
    if (node.type === "text") return expandText(node, citations);
    if (
      !("children" in node) ||
      node.type === "link" ||
      node.type === "linkReference"
    )
      return [node];
    const children = node.children.flatMap(expand);
    if (node.type === "listItem" && appearance === "excerpt")
      return [
        {
          ...node,
          data: { ...node.data, hProperties: { tabIndex: -1 } },
          children,
        },
      ];
    return [{ ...node, children }];
  };
  return () =>
    (root): Branch => ({ ...root, children: root.children.flatMap(expand) });
};
