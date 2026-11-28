import {
  createContext,
  use,
  type ReactElement,
  type ReactNode,
  type ComponentProps,
} from "react";
import ReactMarkdown, { type ExtraProps, type Options } from "react-markdown";
import { citationPlugin, type MarkdownCitation } from "./markdown-citations";
import { resolveMarkdownAction, MarkdownAction } from "./MarkdownAction";
import remarkGfm from "remark-gfm";
import { ScrollArea } from "./ScrollArea";
import {
  parseExternalUrl,
  parseFreeText,
  emptyFreeText,
  type ExternalUrl,
  type FreeText,
  type EntityId,
} from "../lib";
type Conversion = NonNullable<
  NonNullable<Options["remarkRehypeOptions"]>["unknownHandler"]
>;
export type MarkdownParent = Readonly<NonNullable<Parameters<Conversion>[2]>>;
type HtmlElement = Readonly<NonNullable<ExtraProps["node"]>>;
type ContentNode = Readonly<HtmlElement["children"][0]>;
type LinkContent = {
  readonly node?: HtmlElement | undefined;
  readonly href?: ComponentProps<"a">["href"];
  readonly children?: ReactNode;
};
type ImageContent = {
  readonly src?: ComponentProps<"img">["src"];
  readonly alt?: ComponentProps<"img">["alt"];
};
type BlockContent = { readonly children?: ReactNode };
type Props = {
  readonly content: FreeText;
  readonly linkAllowed: (url: ExternalUrl) => "allow" | "deny";
  readonly onOpenLink: (url: ExternalUrl) => void;
  readonly onLocalCitation?: (number: EntityId) => void;
  readonly citations?: readonly MarkdownCitation[];
  readonly scale?: "body" | "reading" | "lead";
  readonly flavor?: "basic" | "gfm";
  readonly appearance?: "plain" | "question";
};
const textScales = {
  body: "markdown-content text-body text-ink",
  reading: "markdown-content text-size-lg text-ink",
  lead: "markdown-content text-size-lg font-semibold text-ink",
} as const;
const MarkdownContext = createContext<Props | undefined>(undefined);
const LinkContext = createContext<"outside" | "inside">("outside");
const MarkdownImage = ({ src, alt }: ImageContent): ReactElement => {
  const props = use(MarkdownContext);
  const nesting = use(LinkContext);
  const url = parseExternalUrl(src);
  const imageText = imageLabel(alt, src);
  return valuePresent(imageText) === "present" &&
    nesting === "outside" &&
    props !== undefined &&
    url.kind === "ok" &&
    props.linkAllowed(url.value) === "allow" ? (
    <MarkdownAction
      onOpen={(): void => {
        props.onOpenLink(url.value);
      }}
    >
      <span>{imageText}</span>
    </MarkdownAction>
  ) : (
    <span>{imageText}</span>
  );
};
const valuePresent = (value: unknown): "present" | "empty" =>
  typeof value === "string" &&
  value.replace(/\p{Default_Ignorable_Code_Point}/gu, "").trim() !== ""
    ? "present"
    : "empty";
const imageLabel = (alt: unknown, src: unknown): FreeText => {
  const label = parseFreeText(valuePresent(alt) === "present" ? alt : src);
  return label.kind === "ok" ? label.value : emptyFreeText;
};
const hasChildren = (
  children: readonly ContentNode[] | undefined,
): "present" | "empty" =>
  children?.some((child) => hasContent(child) === "present") === true
    ? "present"
    : "empty";
const hasContent = (node: ContentNode): "present" | "empty" => {
  if (node.type === "text") return valuePresent(node.value);
  if (node.type !== "element") return "empty";
  if (node.tagName === "img")
    return valuePresent(imageLabel(node.properties.alt, node.properties.src));
  return hasChildren(node.children);
};
const citationNumber = (node: LinkContent["node"]): unknown =>
  node?.properties["dataCitation"];
const MarkdownLink = ({ href, children, node }: LinkContent): ReactElement => {
  const props = use(MarkdownContext);
  const citation = props?.citations?.find(
    (entry) => entry.number === citationNumber(node),
  );
  const url = parseExternalUrl(href);
  const open = resolveMarkdownAction(
    citation,
    url.kind === "ok" ? url.value : undefined,
    props,
  );
  if (hasChildren(node?.children) === "empty" || open === undefined)
    return (
      <LinkContext value="inside">
        <span>{children}</span>
      </LinkContext>
    );
  return (
    <LinkContext value="inside">
      <MarkdownAction citation={citation?.title} onOpen={open}>
        <>{children}</>
      </MarkdownAction>
    </LinkContext>
  );
};
const MarkdownPre = ({ children }: BlockContent): ReactElement =>
  use(MarkdownContext)?.flavor === "gfm" ? (
    <pre className="chat-content font-mono text-code text-ink my-space-3 rounded-md bg-surface-sunken p-space-3">
      {children}
    </pre>
  ) : (
    <pre className="chat-content font-sans my-space-3 rounded-md bg-surface-sunken p-space-3">
      {children}
    </pre>
  );
const MarkdownParagraph = ({ children }: BlockContent): ReactElement =>
  use(MarkdownContext)?.appearance === "question" ? (
    <p className="my-space-1">{children}</p>
  ) : (
    <p className="my-space-3">{children}</p>
  );
const MarkdownH1 = ({ children }: BlockContent): ReactElement => (
  <h1 className="text-title-lg my-space-4">{children}</h1>
);
const MarkdownH2 = ({ children, node }: LinkContent): ReactElement | null =>
  node?.properties.id === "footnote-label" ? null : (
    <h2 className="text-title-md my-space-4">{children}</h2>
  );
const smallHeading = (
  Tag: "h3" | "h4" | "h5" | "h6",
): ((props: BlockContent) => ReactElement) =>
  function MarkdownHeading({ children }: BlockContent): ReactElement {
    return <Tag className="text-title-sm my-space-3">{children}</Tag>;
  };
const MarkdownUl = ({ children }: BlockContent): ReactElement => (
  <ul className="markdown-bullets my-space-3">{children}</ul>
);
type OrderedContent = BlockContent & {
  readonly start?: ComponentProps<"ol">["start"];
};
const MarkdownOl = ({ children, start }: OrderedContent): ReactElement => (
  <ol start={start} className="markdown-ordered my-space-3">
    {children}
  </ol>
);
const MarkdownQuote = ({ children }: BlockContent): ReactElement => (
  <blockquote className="markdown-quote my-space-3">{children}</blockquote>
);
const MarkdownCode = ({ children }: BlockContent): ReactElement =>
  use(MarkdownContext)?.flavor === "gfm" ? (
    <code className="font-mono text-code bg-surface-sunken rounded-sm px-space-1">
      {children}
    </code>
  ) : (
    <code className="font-sans bg-surface-sunken">{children}</code>
  );

const MarkdownTable = ({ children }: BlockContent): ReactElement => (
  <div className="my-space-3">
    <ScrollArea direction="horizontal">
      <table className="border-collapse text-body-sm text-ink">
        {children}
      </table>
    </ScrollArea>
  </div>
);
const MarkdownTh = ({ children }: BlockContent): ReactElement => (
  <th className="px-space-2 py-space-1 text-left text-label text-ink-muted border-b-hairline border-solid border-line-strong">
    {children}
  </th>
);
const MarkdownTd = ({ children }: BlockContent): ReactElement => (
  <td className="px-space-2 py-space-2 border-b-hairline border-solid border-line">
    {children}
  </td>
);
const components = {
  a: MarkdownLink,
  img: MarkdownImage,
  pre: MarkdownPre,
  p: MarkdownParagraph,
  h1: MarkdownH1,
  h2: MarkdownH2,
  h3: smallHeading("h3"),
  h4: smallHeading("h4"),
  h5: smallHeading("h5"),
  h6: smallHeading("h6"),
  ul: MarkdownUl,
  ol: MarkdownOl,
  blockquote: MarkdownQuote,
  code: MarkdownCode,
  "tabl\u0065": MarkdownTable,
  th: MarkdownTh,
  td: MarkdownTd,
  input: (): null => null,
};
export const Markdown = (props: Props): ReactElement => {
  const plugins = props.flavor === "gfm" ? [remarkGfm] : [];
  const citations = props.citations ?? [];
  const body = (
    <ReactMarkdown
      skipHtml
      components={components}
      remarkPlugins={
        citations.length === 0
          ? plugins
          : [...plugins, citationPlugin(citations)]
      }
    >
      {props.content}
    </ReactMarkdown>
  );
  return (
    <MarkdownContext value={props}>
      {props.appearance === "question" ? (
        <div className="markdown-content max-w-full rounded-lg bg-surface-sunken px-space-3 py-space-2 text-body text-ink">
          {body}
        </div>
      ) : (
        <div className={textScales[props.scale ?? "body"]}>{body}</div>
      )}
    </MarkdownContext>
  );
};
