import type { ReactElement, ReactNode, ComponentProps } from "react";
import ReactMarkdown from "react-markdown";
import {
  parseExternalUrl,
  parseFreeText,
  emptyFreeText,
  type ExternalUrl,
  type FreeText,
} from "../lib";
type LinkContent = {
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
};
export const Markdown = (props: Props): ReactElement => (
  <div className="markdown-content text-body text-ink">
    <ReactMarkdown
      skipHtml
      components={{
        a: ({ href, children }: LinkContent): ReactElement => {
          const url = parseExternalUrl(href);
          return url.kind === "ok" &&
            props.linkAllowed(url.value) === "allow" ? (
            <button
              type="button"
              className="inline-block markdown-link text-primary focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
              onClick={(): void => {
                props.onOpenLink(url.value);
              }}
            >
              {children}
            </button>
          ) : (
            <span>{children}</span>
          );
        },
        img: ({ src, alt }: ImageContent): ReactElement => {
          const url = parseExternalUrl(src);
          const label = parseFreeText(
            alt === undefined || alt === "" ? src : alt,
          );
          const imageText = label.kind === "ok" ? label.value : emptyFreeText;
          return url.kind === "ok" &&
            props.linkAllowed(url.value) === "allow" ? (
            <button
              type="button"
              className="inline-block markdown-link text-primary"
              onClick={(): void => {
                props.onOpenLink(url.value);
              }}
            >
              {imageText}
            </button>
          ) : (
            <span>{imageText}</span>
          );
        },
        pre: ({ children }: BlockContent): ReactElement => (
          <pre className="chat-content">{children}</pre>
        ),
      }}
    >
      {props.content}
    </ReactMarkdown>
  </div>
);
