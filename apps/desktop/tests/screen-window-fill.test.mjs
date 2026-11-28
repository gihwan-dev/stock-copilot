import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import {
  auditWith,
  checkedSizes,
  inspectPage,
  launchBrowser,
  scenarioIndex,
  sizes,
  themes,
  withAuditServer,
} from "../../../harness/screen-audit/run.mjs";
import { desktopRoot } from "./paths.mjs";

const page = (body, style = "") =>
  `<!doctype html><html><head><style>html,body{margin:0}*{box-sizing:border-box}${style}</style></head><body>${body}</body></html>`;
const frame = (inner, css = "background:#fff;height:100vh") =>
  `<div data-app-frame style="display:flex;flex-direction:column;width:100%;${css}">${inner}</div>`;

const witnesses = [
  ["frame-missing", page("<div style='background:#fff'>내용</div>")],
  ["frame-unpainted", page(frame("내용", "height:100vh"))],
  ["frame-gap", page(frame("내용", "background:#fff;height:50vh"))],
  [
    "document-scroll-x",
    page(frame("<div style='width:3000px;height:10px'></div>")),
  ],
  [
    "document-scroll-y",
    page(frame("<div style='height:3000px;flex:none'></div>")),
  ],
  [
    "clipped-width",
    page(
      frame(
        "<div data-scroll-viewport style='overflow-x:hidden;overflow-y:auto;width:200px;height:100px'><div style='width:900px;height:10px'></div></div>",
      ),
    ),
  ],
  [
    "pinned-action-hidden",
    page(
      frame(
        "<div style='flex:none;height:2000px'></div><div data-pinned-actions><button>연결하기</button></div>",
        "background:#fff;height:100vh;overflow:hidden",
      ),
    ),
  ],
  [
    "dialog-outside",
    page(
      frame(
        "<div role='dialog' style='position:fixed;top:600px;left:10px;width:200px;height:400px'>창</div>",
      ),
    ),
  ],
];

test("screen audit: each window-fill rule fails its own broken page and passes a full frame", async () => {
  const browser = await launchBrowser();
  try {
    const context = await browser.newContext({ viewport: sizes.minimum });
    const tab = await context.newPage();
    for (const [kind, html] of witnesses) {
      await tab.setContent(html);
      const kinds = (await inspectPage(tab)).map((finding) => finding.kind);
      assert.ok(kinds.includes(kind), `${kind}: ${JSON.stringify(kinds)}`);
    }
    await tab.setContent(page(frame("<p>내용</p>")));
    assert.deepEqual(await inspectPage(tab), []);
    await context.close();
  } finally {
    await browser.close();
  }
});

test("screen audit: every route and state fills the window at every size and theme", async () => {
  const routes = [
    ...readFileSync(join(desktopRoot, "src/app/router.tsx"), "utf8").matchAll(
      /path: "([^"]+)"/gu,
    ),
  ].map((match) => match[1]);
  const results = await withAuditServer(async (target) => {
    const index = await scenarioIndex(target);
    for (const route of routes)
      assert.ok(index.paths.includes(route), `장면 없는 경로: ${route}`);
    assert.ok(index.names.length >= 20, `장면 수: ${index.names.length}`);
    return auditWith(target, {
      sizes: [...checkedSizes],
      themes: [...themes],
    });
  });
  const failing = results.filter((row) => row.findings.length !== 0);
  assert.deepEqual(
    failing.map(
      (row) =>
        `${row.scenario} ${row.size} ${row.theme}: ${JSON.stringify(row.findings)}`,
    ),
    [],
  );
});

test("minimum window supports keyboard travel between dashboard, briefing reader and chat", async () => {
  await withAuditServer(async ({ browser, url }) => {
    const context = await browser.newContext({ viewport: sizes.minimum });
    try {
      const tab = await context.newPage();
      tab.setDefaultTimeout(15000);
      await tab.goto(`${url}?s=briefing-latest`);
      await tab.waitForFunction(() => globalThis.auditState === "ready");
      const nav = tab.getByRole("navigation", { name: "화면 이동" });
      const dashboard = nav.getByRole("button", { name: "대시보드" });
      await dashboard.click();
      const morning = tab
        .locator("main .markdown-content")
        .filter({ hasText: "밤사이 지표" });
      const morningTail = "오늘 아침 점검을 마칩니다.";
      const fontSize = (body) =>
        body
          .locator("p")
          .first()
          .evaluate((p) =>
            Number.parseFloat(globalThis.getComputedStyle(p).fontSize),
          );
      const previewSize = await fontSize(morning);
      const finalParagraph = morning.getByText(morningTail, { exact: true });
      await finalParagraph.scrollIntoViewIfNeeded();
      assert.ok((await morning.innerText()).includes(morningTail));
      await dashboard.focus();
      await tab.keyboard.press("Tab");
      await tab.keyboard.press("Enter");
      await morning.getByText(morningTail, { exact: true }).waitFor();
      const readingSize = await fontSize(morning);
      assert.ok(
        readingSize > previewSize,
        `본문 ${readingSize}px, 미리보기 ${previewSize}px`,
      );
      const preMarket = tab
        .locator("main .markdown-content")
        .filter({ hasText: "미국장 전 점검을 마칩니다." });
      assert.equal(await fontSize(preMarket), readingSize);
      const expand = tab.getByRole("button", { name: "크게 보기" });
      assert.equal(await expand.count(), 0);
      assert.equal(await tab.getByRole("dialog").count(), 0);
      assert.equal(await morning.getByRole("link", { name: "[1]" }).count(), 1);
      assert.ok((await morning.innerText()).includes("[2]"));
      assert.equal(await morning.getByRole("link", { name: "[2]" }).count(), 0);
      const history = tab.getByRole("button", { name: "지난 브리핑 보기" });
      await history.focus();
      await tab.keyboard.press("Enter");
      await tab
        .getByRole("heading", { name: "지난 브리핑", exact: true })
        .waitFor();
      assert.equal(await fontSize(preMarket), readingSize);
      assert.equal(await morning.count(), 0);
      assert.equal(await expand.count(), 0);
      assert.equal(await tab.getByRole("dialog").count(), 0);
      await tab.getByText("연합 기사", { exact: true }).waitFor();
      await tab.getByText("원문 없음", { exact: true }).waitFor();
      await tab.getByText("2026-11-24 22:30", { exact: true }).waitFor();
      assert.equal(
        await tab.getByText("자료 확인 시각", { exact: true }).count(),
        2,
      );
      assert.equal(
        await tab.getByText("2026-11-24 22:15", { exact: true }).count(),
        2,
      );
      await tab.getByRole("button", { name: "현황으로" }).click();
      await morning.waitFor();
      await nav.getByRole("button", { name: "채팅" }).click();
      await tab
        .getByText("보유 종목을 확인했습니다.", { exact: true })
        .waitFor();
      assert.equal(await tab.locator("main textarea").count(), 1);
      await nav.getByRole("button", { name: "설정", exact: true }).click();
      await tab.getByRole("button", { name: "이전 화면으로" }).click();
      await tab.getByRole("heading", { name: "AI 채팅" }).waitFor();
      await dashboard.click();
      await tab.getByRole("heading", { name: "내 현황" }).waitFor();
      assert.equal(await tab.locator("main textarea").count(), 0);

      await tab.goto(`${url}?s=chat-long`);
      await tab.waitForFunction(() => globalThis.auditState === "ready");
      const messages = tab
        .locator("main [data-scroll-viewport]")
        .filter({ hasText: "대화 36 — 보유 종목을 확인했습니다." })
        .first();
      const atLastMessage = (scroller) =>
        scroller.scrollHeight > scroller.clientHeight &&
        scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop < 3;
      await tab.waitForFunction(atLastMessage, await messages.elementHandle());
      const input = tab.getByRole("textbox", { name: "메시지" });
      await input.fill("새 응답 위치를 확인해 줘");
      await input.press("Enter");
      await tab.getByText("스크롤 확인용 새 응답입니다.").waitFor();
      await tab.waitForFunction(atLastMessage, await messages.elementHandle());
      const before = await input.boundingBox();
      assert.ok(
        before.y >= 0 && before.y + before.height <= sizes.minimum.height,
      );
      await messages.evaluate((el) => el.scrollTo({ top: 0 }));
      assert.deepEqual(await input.boundingBox(), before);
      await tab.setViewportSize(sizes.wide);
      for (const name of ["대시보드", "브리핑", "채팅", "설정"]) {
        await nav
          .getByText(name, { exact: true })
          .waitFor({ state: "visible" });
      }
    } finally {
      await context.close();
    }
  });
});

test("stock list stays right of main and leaves the composer accessible when opened from the rail", async () => {
  await withAuditServer(async ({ browser, url }) => {
    const tab = await browser.newPage({ viewport: sizes.wide });
    try {
      tab.setDefaultTimeout(15000);
      await tab.goto(`${url}?s=home-ready`);
      const nav = tab.getByRole("navigation", { name: "화면 이동" });
      const main = tab.getByRole("main");
      const list = tab
        .getByRole("complementary", { name: "종목 목록" })
        .filter({ hasText: "예시바이오" });
      await list.waitFor();
      const rightOf = async (right, left) => {
        const a = await left.boundingBox(),
          b = await right.boundingBox();
        assert.ok(a && b && b.x >= a.x + a.width - 1, JSON.stringify({ a, b }));
      };
      await rightOf(main, nav);
      await rightOf(list, main);
      await list.getByRole("button", { name: "상세 보기" }).first().click();
      await main.getByText("일봉 차트", { exact: true }).first().waitFor();
      await main
        .getByRole("button", { name: "닫기", exact: true })
        .first()
        .click();
      await main.getByRole("heading", { name: "내 현황" }).waitFor();
      await nav.getByRole("button", { name: "채팅" }).click();
      const input = main.getByRole("textbox", { name: "메시지" });
      await input.waitFor();
      await tab.setViewportSize(sizes.minimum);
      const toggle = tab.getByRole("button", { name: "종목 목록 펼치기" });
      await toggle.waitFor();
      await rightOf(toggle, main);
      await toggle.focus();
      await tab.keyboard.press("Enter");
      await list.waitFor();
      await rightOf(list, main);
      await rightOf(main, nav);
      await rightOf(list, input);
      await input.click({ trial: true });
      await tab.keyboard.press("Escape");
      await list.waitFor({ state: "hidden" });
      await toggle.click();
      await list.getByRole("button", { name: "종목 목록 닫기" }).click();
      await toggle.waitFor();
      assert.equal(await toggle.getAttribute("aria-expanded"), "false");
    } finally {
      await tab.close();
    }
  });
});
