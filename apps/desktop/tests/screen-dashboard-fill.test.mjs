import assert from "node:assert/strict";
import test from "node:test";
import { sizes, withAuditServer } from "../../../harness/screen-audit/run.mjs";

const measure = () => {
  const tolerance = 1;
  const main = globalThis.document.querySelector("main");
  if (main === null) return { error: "main 없음" };
  const verticalScrollers = [
    ...main.querySelectorAll("[data-scroll-viewport]"),
  ].filter((node) => node.scrollHeight > node.clientHeight + tolerance);
  const looseScrollers = verticalScrollers
    .filter((node) => node.closest("main section") === null)
    .map((node) => `${node.scrollHeight}/${node.clientHeight}`);
  const rows = [...main.querySelectorAll(".grid")]
    .filter((grid) => grid.closest("section") === null)
    .map((grid) =>
      [...grid.children].map((item) => {
        const card = item.querySelector("section");
        return {
          item: Math.round(item.getBoundingClientRect().height),
          card:
            card === null
              ? -1
              : Math.round(card.getBoundingClientRect().height),
        };
      }),
    );
  const cardScrollers = verticalScrollers.filter(
    (node) => node.closest("main section") !== null,
  );
  const stickyHeaders = [...main.querySelectorAll("section thead th")].map(
    (cell) => globalThis.getComputedStyle(cell).position,
  );
  return {
    main: { scroll: main.scrollHeight, client: main.clientHeight },
    mainBottom: Math.round(main.getBoundingClientRect().bottom),
    windowHeight: globalThis.innerHeight,
    looseScrollers,
    rows,
    cardScrollers: cardScrollers.length,
    stickyHeaders,
  };
};

const pause = (ms) => new Promise((done) => setTimeout(done, ms));

const visit = async (page, address, attempt = 1) => {
  const response = await page.goto(address).catch((error) => error);
  if (response instanceof Error || !response.ok()) {
    if (attempt >= 5) throw new Error(`열지 못함: ${address}`);
    await pause(500 * attempt);
    return visit(page, address, attempt + 1);
  }
  return response;
};

const open = async ({ browser, url }, scenario, size) => {
  const context = await browser.newContext({
    viewport: sizes[size],
    colorScheme: "light",
    locale: "ko-KR",
    timezoneId: "Asia/Seoul",
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  await visit(page, `${url}?s=${encodeURIComponent(scenario)}`);
  await page.waitForFunction(() => globalThis.auditState !== undefined, null, {
    timeout: 15000,
  });
  assert.equal(await page.evaluate(() => globalThis.auditState), "ready");
  await page.evaluate(() => globalThis.document.fonts.ready);
  return { context, page };
};

test("home dashboard: the main view never scrolls, same-row cards share one height, and overflow scrolls inside cards", async () => {
  const results = await withAuditServer(async (target) => {
    const rows = [];
    for (const scenario of ["home-ready", "home-loading", "home-error"])
      for (const size of ["wide", "medium", "minimum"]) {
        const { context, page } = await open(target, scenario, size);
        try {
          rows.push({ scenario, size, ...(await page.evaluate(measure)) });
        } finally {
          await context.close();
        }
      }
    return rows;
  });
  for (const row of results) {
    const where = `${row.scenario} ${row.size}`;
    assert.equal(row.error, undefined, where);
    assert.ok(
      row.main.scroll <= row.main.client + 1,
      `${where}: 메인 화면이 스크롤된다 ${row.main.scroll}/${row.main.client}`,
    );
    assert.ok(row.mainBottom <= row.windowHeight + 1, `${where}: 창 밖으로`);
    assert.deepEqual(row.looseScrollers, [], `${where}: 카드 밖 스크롤`);
    assert.ok(row.rows.length >= 1, `${where}: 대시보드 줄 없음`);
    for (const items of row.rows) {
      const heights = new Set(items.map((item) => item.item));
      assert.equal(
        heights.size,
        1,
        `${where}: 줄 높이 ${JSON.stringify(items)}`,
      );
      for (const item of items)
        if (item.card !== -1)
          assert.ok(
            Math.abs(item.card - item.item) <= 1,
            `${where}: 카드가 칸을 채우지 않는다 ${JSON.stringify(items)}`,
          );
    }
  }
  const ready = results.filter((row) => row.scenario === "home-ready");
  for (const row of ready) {
    assert.equal(row.rows.length, 3, `${row.size}: 줄 수`);
    assert.deepEqual(
      row.rows.map((items) => items.length),
      [2, 2, 2],
      row.size,
    );
    assert.ok(row.stickyHeaders.length > 0, row.size);
    assert.ok(
      row.stickyHeaders.every((position) => position === "sticky"),
      `${row.size}: 표 머리글 고정 ${row.stickyHeaders.join(",")}`,
    );
  }
  const minimum = ready.find((row) => row.size === "minimum");
  assert.ok(minimum.cardScrollers >= 1, "작은 창에서는 카드 안에서 스크롤");
});
