import { expect, test } from "vitest";
import {
  parseEntityId,
  parseExternalUrl,
  parseFreeText,
  type Result,
} from "../lib";
import { citationPlugin } from "./markdown-citations";

const must = <T, E>(result: Result<T, E>): T =>
  result.kind === "ok" ? result.value : expect.unreachable();

test("adjacent numbers use their recorded URLs while other numbers, code and existing links retain their contents", () => {
  const url = must(parseExternalUrl("https://example.com/source?a=1&b=2"));
  const transform = citationPlugin([
    {
      number: must(parseEntityId("1")),
      url,
      title: must(parseFreeText("Article")),
    },
  ])();
  const code = { type: "inlineCode", value: "[1]" } as const;
  const link = {
    type: "link",
    url: "https://example.com/explicit",
    children: [{ type: "text" as const, value: "[1]" }],
  };
  const tree = transform({
    type: "root",
    children: [
      {
        type: "paragraph",
        children: [
          { type: "strong", children: [{ type: "text", value: "[2][1]" }] },
          code,
          { ...link, type: "link" },
        ],
      },
    ],
  });
  expect(tree.children).toStrictEqual([
    {
      type: "paragraph",
      children: [
        {
          type: "strong",
          children: [
            { type: "text", value: "[2]" },
            {
              type: "link",
              url,
              children: [{ type: "text", value: "[1]" }],
              data: { hProperties: { dataCitation: "1" } },
            },
          ],
        },
        code,
        link,
      ],
    },
  ]);
});
