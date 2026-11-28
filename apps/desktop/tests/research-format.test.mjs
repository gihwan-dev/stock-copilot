import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { createServer } from "vite";
import { desktopRoot } from "./paths.mjs";

let vite, reportMarkdown;
before(async () => {
  vite = await createServer({
    root: desktopRoot,
    configFile: false,
    server: { middlewareMode: true, watch: null },
  });
  ({ reportMarkdown } = await vite.ssrLoadModule(
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
