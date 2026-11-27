import type { BriefingSourceView } from "./estimate-views";
import { text } from "../lib";
import { expect, test } from "vitest";
import { settleBriefings } from "./briefing-schema";
import { wire, type Wire } from "./schema";
import { responses } from "./testing/responses";

const recorded = responses.getBriefings.latest;

const sourcesOf = (sources: readonly Wire[]): readonly BriefingSourceView[] => {
  const result = settleBriefings({
    status: "ok",
    data: wire({ ...responses.getBriefings, latest: { ...recorded, sources } }),
  });
  if (result.kind === "err") return expect.unreachable();
  const outcome = result.value.latest?.outcome;
  return outcome?.kind === "completed" ? outcome.sources : expect.unreachable();
};

test("citation numbers and publishers follow the stored references when the wire order changes", () => {
  const first = recorded.sources[0];
  const sources = sourcesOf([
    wire({ ...first, cite: "2", url: "https://example.com/two" }),
    wire({ ...first, cite: "1", url: "https://example.com/one" }),
  ]);
  expect(
    sources.map((source) => [source.cite, source.url, source.publisher]),
  ).toStrictEqual([
    ["2", "https://example.com/two", text("source.yonhap", "ko")],
    ["1", "https://example.com/one", text("source.yonhap", "ko")],
  ]);
});

test("an older wire record keeps its prompt order even when a source has no URL", () => {
  const sources = sourcesOf([
    wire({ title: "No URL", kind: "web", url: null }),
    wire({ title: "Article", kind: "news", url: "https://example.com/a" }),
  ]);
  expect(sources.map((source) => [source.cite, source.url])).toStrictEqual([
    ["1", undefined],
    ["2", "https://example.com/a"],
  ]);
});

test("only collected HTTPS URLs are candidates for the system opener", () => {
  const urls = [
    "javascript:alert(1)",
    "http://example.com/a",
    "file:///etc/hosts",
    "data:text/html,x",
    null,
  ];
  const sources = sourcesOf(
    urls.map((url) => wire({ title: "Source", url, kind: "news" })),
  );
  expect(sources).toHaveLength(urls.length);
  expect(sources.every((source) => source.url === undefined)).toBe(true);
});

test.each([
  [
    "Kind",
    "source.kind-disclosure",
    "https://kind.krx.co.kr/common/disclsviewer.do?acptno=20261127000123",
  ] as const,
  [
    "Dart",
    "source.dart-disclosure-api",
    "https://dart.fss.or.kr/dsaf001/main.do?rcpNo=20261127000123",
  ] as const,
])(
  "a stored %s disclosure keeps its publisher, source URL and observation time",
  (provider, key, url) => {
    const at = "2026-11-27T08:15:00+09:00";
    const [source] = sourcesOf([
      wire({
        title: "Disclosure",
        kind: "disclosure",
        origin: { Disclosure: provider },
        observed_at: at,
        url,
      }),
    ]);
    expect(source).toMatchObject({
      publisher: text(key, "ko"),
      url,
      observedAt: at,
      cite: "1",
    });
  },
);
