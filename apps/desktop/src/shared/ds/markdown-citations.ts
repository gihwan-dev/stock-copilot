import type { MarkdownParent as Parent } from "./Markdown";
import type { EntityId, ExternalUrl, FreeText } from "../lib";

export type MarkdownCitation = {
  readonly number: EntityId;
  readonly url: ExternalUrl;
  readonly title: FreeText;
};
type Node = Readonly<Parent | Parent["children"][0]>;
type Root = Extract<Parent, { type: "root" }>;
type Text = Extract<Node, { type: "text" }>;
type Link = Extract<Node, { type: "link" }>;
type Branch = {
  readonly type: Parent["type"];
  readonly children: readonly (Node | Branch)[];
};

const expandText = (
  node: Text,
  citations: readonly MarkdownCitation[],
): readonly (Text | Link)[] =>
  node.value
    .split(/(\[[1-9]\d*\])/u)
    .filter((value) => value.length > 0)
    .map((value): Text | Link => {
      const label: Text = { type: "text", value };
      const ref = citations.find((entry) => `[${entry.number}]` === value);
      return ref === undefined
        ? label
        : {
            type: "link",
            url: ref.url,
            children: [label],
            data: { hProperties: { dataCitation: ref.number } },
          };
    });

export const citationPlugin = (
  citations: readonly MarkdownCitation[],
): (() => (root: Root) => Branch) => {
  const expand = (node: Node): readonly (Node | Branch)[] => {
    if (node.type === "text") return expandText(node, citations);
    if (
      !("children" in node) ||
      node.type === "link" ||
      node.type === "linkReference"
    )
      return [node];
    return [{ ...node, children: node.children.flatMap(expand) }];
  };
  return () =>
    (root): Branch => ({ ...root, children: root.children.flatMap(expand) });
};
