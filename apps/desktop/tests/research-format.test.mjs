import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { createServer } from "vite";
import { desktopRoot } from "./paths.mjs";

let vite, reportMarkdown, reportParts;
before(async () => {
  vite = await createServer({
    root: desktopRoot,
    configFile: false,
    server: { middlewareMode: true, watch: null },
  });
  ({ reportMarkdown, reportParts } = await vite.ssrLoadModule(
    "/src/features/research/model/report-markdown.ts",
  ));
});
after(() => vite?.close());

test("cached single-line claims remain in order with each original citation", () => {
  const lines = Array.from(
    { length: 26 },
    (_, index) => `관측 자료 [${index + 1}].`,
  );
  assert.equal(
    reportMarkdown(lines.join("\r\n")),
    lines.map((line) => `- ${line}`).join("\n"),
  );
});

test("the three candidate labels read as distinct paragraphs", () => {
  assert.equal(
    reportMarkdown(
      "이유 수주가 늘었다 [1]\n위험:원가가 올랐다 [2]\n확인 조건 재고를 본다 [1]",
    ),
    "**이유** 수주가 늘었다 [1]\n\n**위험** 원가가 올랐다 [2]\n\n**확인 조건** 재고를 본다 [1]",
  );
});

test("preserves authored Markdown and ordinary wrapped paragraphs", () => {
  for (const input of [
    "### 핵심\n\n수주가 늘었다 [1]",
    "- 수주가 늘었다 [1]\n- 원가가 올랐다 [2]",
    "> 수주가 늘었다 [1]\n원가가 올랐다 [2]",
    "    예시 코드 [1]\n이어지는 문장 [2]",
    "한 문단의 앞부분\n뒷부분 [1]",
  ])
    assert.equal(reportMarkdown(input), input);
});

test("only long legacy reports fold, with every original citation after expansion", () => {
  const claims = Array.from(
    { length: 26 },
    (_, i) => `매출이 변했다 [${i + 1}]`,
  );
  const body = claims.join("\n");
  const excerpt = reportParts(body);
  assert.equal(excerpt.kind, "excerpt");
  assert.equal(excerpt.first.split("\n").length, 6);
  assert.equal(excerpt.remaining, "20");
  assert.equal(excerpt.first + "\n" + excerpt.rest, reportMarkdown(body));
  const list = claims.map((line) => "- " + line).join("\n");
  assert.deepEqual(reportParts(list), { kind: "document", body: list });
});

test("a new report keeps the conclusion, analysis and conditions in source order", () => {
  const blocks = [
    "### 핵심\n\n매출 증가 [26].",
    "### 판단 근거\n\n- 수주 증가 [2].",
    "### 확인 조건\n\n원가 상승 [7].",
  ];
  assert.deepEqual(reportParts(blocks.join("\n\n")), {
    kind: "sections",
    lead: blocks[0],
    evidence: blocks[1],
    conditions: blocks[2],
  });
  const fragment = blocks.slice(1).join("\n\n");
  assert.deepEqual(reportParts(fragment), { kind: "document", body: fragment });
});
