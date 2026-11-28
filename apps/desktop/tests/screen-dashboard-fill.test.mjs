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
          top: Math.round(item.getBoundingClientRect().top),
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
  if (scenario === "home-research-refresh") await page.clock.install();
  await visit(page, `${url}?s=${encodeURIComponent(scenario)}`);
  await page.waitForFunction(() => globalThis.auditState !== undefined, null, {
    timeout: 15000,
  });
  assert.equal(await page.evaluate(() => globalThis.auditState), "ready");
  await page.evaluate(() => globalThis.document.fonts.ready);
  return { context, page };
};

test("대시보드의 조사 바로가기와 마지막 근거까지 키보드로 이동한다", async () => {
  const results = await withAuditServer(async (target) => {
    const rows = [];
    for (const scenario of ["home-ready", "home-loading", "home-error"])
      for (const size of ["wide", "medium", "minimum"]) {
        const { context, page } = await open(target, scenario, size);
        try {
          if (scenario === "home-ready") {
            const skip = page
              .getByRole("button", { name: "넘기기", exact: true })
              .first();
            const closed = await skip.getAttribute("aria-controls");
            if (closed !== null)
              assert.equal(
                await page
                  .locator("[id=" + JSON.stringify(closed) + "]")
                  .isVisible(),
                false,
              );
            const candidate = page.locator("#candidates");
            const name = await candidate
              .getByText("예시전력", { exact: true })
              .boundingBox();
            const action = await candidate
              .getByRole("button", { name: "종목 상세" })
              .boundingBox();
            assert.ok(
              name.y < action.y + action.height &&
                action.y < name.y + name.height,
            );
            for (const [topic, label] of [
              ["holdings", "보유 자산 평가"],
              ["candidates", "새 종목 후보"],
              ["economy", "경제와 산업 동향"],
            ]) {
              const jump = page.getByRole("button", {
                name: label,
                exact: true,
              });
              await jump.hover();
              assert.equal(
                await jump.evaluate(
                  (el) => globalThis.getComputedStyle(el).backgroundColor,
                ),
                "rgba(0, 0, 0, 0)",
              );
              await jump.focus();
              await page.keyboard.press("Enter");
              await page.waitForFunction(
                (id) => globalThis.document.activeElement.id === id,
                topic,
              );
              if (topic === "holdings") {
                await page
                  .locator("#holdings")
                  .getByText("005930", { exact: true })
                  .waitFor();
                assert.equal(
                  await page
                    .locator("#holdings")
                    .getByRole("button", { name: "원문 열기" })
                    .count(),
                  0,
                );
                await page
                  .locator("#holdings")
                  .getByRole("link", { name: "[1]", exact: true })
                  .press("Enter");
                await page.waitForFunction(
                  () => globalThis.document.activeElement.id === "holdings-1",
                );
                continue;
              }
              const source = page
                .locator("#" + topic)
                .getByRole("button", { name: "원문 열기" });
              await source.focus();
              const bounds = await source.boundingBox();
              assert.ok(
                bounds.y >= 0 && bounds.y + bounds.height <= sizes[size].height,
                topic,
              );
            }
            const last = page.getByText(
              "설비 투자가 큰 제조업에서는 이자 비용과 신규 수주를 함께 살펴야 합니다.",
              { exact: true },
            );
            const rect = await last.boundingBox();
            assert.ok(
              rect.y >= 0 && rect.y + rect.height <= sizes[size].height,
              "마지막 경제 분석 문단",
            );
          }
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
    assert.ok(row.looseScrollers.length <= 1, where);
    assert.ok(row.rows.length >= 1, where);
  }
  const ready = results.filter((row) => row.scenario === "home-ready");
  for (const row of ready) {
    assert.equal(row.looseScrollers.length, 1, row.size);
    assert.deepEqual(
      row.rows.map((items) => items.length),
      [2, 2],
      row.size,
    );
    const [summary] = row.rows;
    if (row.size === "wide") {
      assert.equal(summary[0].top, summary[1].top);
      assert.equal(summary[0].item, summary[1].item, row.size);
    } else
      assert.ok(summary[1].top >= summary[0].top + summary[0].item, row.size);
    assert.ok(
      summary.every((item) => item.card > 0 && item.card <= item.item + 1),
      row.size,
    );
    assert.ok(row.stickyHeaders.length > 0, row.size);
    assert.ok(
      row.cardScrollers === 0 &&
        row.stickyHeaders.every((position) => position === "static"),
      `${row.size}: 표와 본문이 같은 뷰포트에서 스크롤 ${row.stickyHeaders.join(",")}`,
    );
  }
});

test("한 조사가 실패해도 보관된 분석과 근거는 열리고 갱신 중인 조사는 다시 조회한다", async () => {
  await withAuditServer(async (target) => {
    const { context, page } = await open(
      target,
      "home-research-refresh",
      "minimum",
    );
    try {
      const holdings = page.locator("#holdings");
      await holdings.getByText("이번 조사 결과를 불러오지 못했어요.").waitFor();
      await holdings.getByText("예시전자 분기보고서").waitFor();
      await page
        .locator("#candidates")
        .getByText("아직 조사 결과가 없어요.")
        .waitFor();
      const economy = page.locator("#economy");
      const updating = economy.getByText("새 자료를 확인하고 있어요.");
      await updating.waitFor();
      await page.clock.fastForward(5100);
      await updating.waitFor({ state: "detached", timeout: 10000 });
      await economy.getByText("산업 동향 조사").waitFor();
      await page.clock.fastForward(300_100);
      await page
        .locator("#candidates")
        .getByText("012345", { exact: true })
        .waitFor();
      assert.equal(
        await page
          .locator("#candidates")
          .getByRole("button", { name: "종목 상세" })
          .count(),
        0,
      );
      await holdings
        .getByText("이번 조사 결과를 불러오지 못했어요.")
        .waitFor({ state: "detached" });
    } finally {
      await context.close();
    }
  });
});

test("운영 해시 라우터에서 조사 이동·설정 왕복·새로고침이 현재 경로를 유지한다", async () => {
  await withAuditServer(async ({ browser, url }) => {
    const page = await browser.newPage({ viewport: sizes.minimum });
    try {
      await page.goto(url + "?s=home-ready&hash-route#/");
      const jump = page.getByRole("button", {
        name: "새 종목 후보",
        exact: true,
      });
      await jump.click();
      assert.equal(new URL(page.url()).hash, "#/");
      await jump.focus();
      await page.keyboard.press("Enter");
      await page.waitForFunction(
        () => globalThis.document.activeElement.id === "candidates",
      );
      await page.getByRole("button", { name: "설정", exact: true }).click();
      await page.waitForFunction(
        () => globalThis.location.hash === "#/settings",
      );
      await page.goBack();
      await jump.waitFor();
      assert.equal(new URL(page.url()).hash, "#/");
      await page.reload();
      await jump.waitFor();
      assert.equal(new URL(page.url()).hash, "#/");
      await page
        .locator("#candidates")
        .getByRole("button", { name: "종목 상세" })
        .press("Enter");
      await page
        .getByRole("main")
        .getByText("예시전력", { exact: true })
        .waitFor();
      await page.getByRole("button", { name: "닫기", exact: true }).click();
      await page
        .getByRole("main")
        .getByRole("button", { name: "예시전자", exact: true })
        .press("Enter");
      await page
        .getByRole("main")
        .getByText("005930", { exact: true })
        .waitFor();
    } finally {
      await page.close();
    }
  });
});
