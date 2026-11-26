import {
  createContext,
  use,
  type ReactElement,
  type ReactNode,
  type ComponentProps,
} from "react";
import ReactMarkdown, { type Options } from "react-markdown";
import remarkGfm from "remark-gfm";
import { ScrollArea } from "./ScrollArea";
import {
  parseExternalUrl,
  parseFreeText,
  emptyFreeText,
  type ExternalUrl,
  type FreeText,
} from "../lib";
type ContentNode = {
  readonly type: string;
  readonly value?: string;
  readonly tagName?: string;
  readonly properties?: Readonly<Record<string, unknown>> & {
    readonly alt?: unknown;
    readonly src?: unknown;
  };
  readonly children?: readonly ContentNode[];
};
type LinkContent = {
  readonly node?: { readonly children: readonly ContentNode[] } | undefined;
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
  readonly flavor?: "basic" | "gfm";
  readonly appearance?: "plain" | "question";
};
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
    <button
      type="button"
      className="inline-block markdown-link text-left text-primary focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
      onClick={(): void => {
        props.onOpenLink(url.value);
      }}
    >
      {imageText}
    </button>
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
    return valuePresent(imageLabel(node.properties?.alt, node.properties?.src));
  return hasChildren(node.children);
};
const MarkdownLink = ({ href, children, node }: LinkContent): ReactElement => {
  const props = use(MarkdownContext);
  const url = parseExternalUrl(href);
  return (
    <LinkContext value="inside">
      {hasChildren(node?.children) === "present" &&
      props !== undefined &&
      url.kind === "ok" &&
      props.linkAllowed(url.value) === "allow" ? (
        <button
          type="button"
          className="inline-block markdown-link text-left text-primary focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
          onClick={(): void => {
            props.onOpenLink(url.value);
          }}
        >
          {children}
        </button>
      ) : (
        <span>{children}</span>
      )}
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
type HeadingContent = BlockContent & {
  readonly node?:
    { readonly properties?: ContentNode["properties"] } | undefined;
};
const MarkdownH2 = ({ children, node }: HeadingContent): ReactElement | null =>
  node?.properties?.["id"] === "footnote-label" ? null : (
    <h2 className="text-title-md my-space-4">{children}</h2>
  );
const MarkdownH3 = ({ children }: BlockContent): ReactElement => (
  <h3 className="text-title-sm my-space-3">{children}</h3>
);
const MarkdownH4 = ({ children }: BlockContent): ReactElement => (
  <h4 className="text-title-sm my-space-3">{children}</h4>
);
const MarkdownH5 = ({ children }: BlockContent): ReactElement => (
  <h5 className="text-title-sm my-space-3">{children}</h5>
);
const MarkdownH6 = ({ children }: BlockContent): ReactElement => (
  <h6 className="text-title-sm my-space-3">{children}</h6>
);
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
const MarkdownTask = (): null => null;
const components = {
  a: MarkdownLink,
  img: MarkdownImage,
  pre: MarkdownPre,
  p: MarkdownParagraph,
  h1: MarkdownH1,
  h2: MarkdownH2,
  h3: MarkdownH3,
  h4: MarkdownH4,
  h5: MarkdownH5,
  h6: MarkdownH6,
  ul: MarkdownUl,
  ol: MarkdownOl,
  blockquote: MarkdownQuote,
  code: MarkdownCode,
  "tabl\u0065": MarkdownTable,
  th: MarkdownTh,
  td: MarkdownTd,
  input: MarkdownTask,
};
const basicPlugins: Options["remarkPlugins"] = [];
const gfmPlugins: Options["remarkPlugins"] = [remarkGfm];
const Body = (props: {
  readonly content: FreeText;
  readonly flavor: "basic" | "gfm";
}): ReactElement => (
  <ReactMarkdown
    skipHtml
    components={components}
    remarkPlugins={props.flavor === "gfm" ? gfmPlugins : basicPlugins}
  >
    {props.content}
  </ReactMarkdown>
);
export const Markdown = (props: Props): ReactElement => (
  <MarkdownContext value={props}>
    {props.appearance === "question" ? (
      <div className="markdown-content max-w-full rounded-lg bg-surface-sunken px-space-3 py-space-2 text-body text-ink">
        <Body content={props.content} flavor={props.flavor ?? "basic"} />
      </div>
    ) : (
      <div className="markdown-content text-body text-ink">
        <Body content={props.content} flavor={props.flavor ?? "basic"} />
      </div>
    )}
  </MarkdownContext>
);
