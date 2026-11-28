import assert from "node:assert/strict";
import test from "node:test";
import { sizes, withAuditServer } from "../../../harness/screen-audit/run.mjs";

const dashboardSizes = {
  ...sizes,
  fullhd: { width: 1920, height: 1080 },
  qhd: { width: 2560, height: 1440 },
};

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
  const card = (title) =>
    [...main.querySelectorAll("section > header h3")]
      .find((heading) => heading.textContent === title)
      ?.closest("section");
  const bounds = (element) => {
    if (!element) return undefined;
    const { x, y, width, height } = element.getBoundingClientRect();
    return { x, y, width, height };
  };
  const summary = card("총평가");
  const briefing =
    card("아침 브리핑") ??
    card("브리핑을 불러오는 중이에요") ??
    card("브리핑을 불러오지 못했어요");
  const hasCurrencyTable = [
    ...(summary?.querySelectorAll("caption") ?? []),
  ].some((caption) => caption.textContent === "통화별 평가");
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
    hasCurrencyTable,
    area: bounds(main),
    summary: bounds(summary),
    briefing: bounds(briefing),
    prose: bounds(briefing?.querySelector(".markdown-content")),
    holdings: bounds(card("보유 종목")),
    signals: bounds(main.querySelector("#signals")),
    reports: ["holdings", "candidates", "economy"].map((id) =>
      bounds(main.querySelector("#" + id)),
    ),
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
    viewport: dashboardSizes[size],
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
      for (const size of scenario === "home-ready"
        ? ["qhd", "fullhd", "wide", "medium", "minimum"]
        : ["wide", "medium", "minimum"]) {
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
            const shortcuts = [
              ["allocation", "종목·섹터별 비중"],
              ...(["minimum", "medium"].includes(size)
                ? [
                    ["positions", "보유 내역"],
                    ["daily-briefing", "오늘 브리핑"],
                    ["signals", "신호 보기"],
                  ]
                : []),
            ];
            for (const [id, label] of shortcuts) {
              await page
                .getByRole("button", { name: label, exact: true })
                .press("Enter");
              await page.waitForFunction(
                (id) => globalThis.document.activeElement.id === id,
                id,
              );
              assert.ok(
                (await page.locator("#" + id).boundingBox()).y <
                  dashboardSizes[size].height,
              );
            }
            const sources = page
              .locator("#daily-briefing")
              .getByRole("button", { name: "브리핑 근거 자료" });
            assert.equal(await sources.getAttribute("aria-expanded"), "false");
            await sources.press("Enter");
            assert.equal(await sources.getAttribute("aria-expanded"), "true");
            await sources.press("Enter");
            await page
              .locator("#daily-briefing")
              .getByText("만든 시각", { exact: true })
              .waitFor();
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
              const disclosure = page.locator("#" + topic).getByRole("button", {
                name: "근거 자료 1건",
              });
              assert.equal(
                await disclosure.getAttribute("aria-expanded"),
                "false",
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
                assert.equal(
                  await disclosure.getAttribute("aria-expanded"),
                  "true",
                );
                await disclosure.press("Space");
                assert.equal(
                  await disclosure.getAttribute("aria-expanded"),
                  "false",
                );
                continue;
              }
              await disclosure.press("Enter");
              const source = page
                .locator("#" + topic)
                .getByRole("button", { name: "원문 열기" });
              await source.focus();
              const bounds = await source.boundingBox();
              assert.ok(
                bounds.y >= 0 &&
                  bounds.y + bounds.height <= dashboardSizes[size].height,
                topic,
              );
            }
            const last = page.getByText(
              "설비 투자가 큰 제조업에서는 이자 비용과 신규 수주를 함께 살펴야 합니다.",
              { exact: true },
            );
            const rect = await last.boundingBox();
            assert.ok(
              rect.y >= 0 &&
                rect.y + rect.height <= dashboardSizes[size].height,
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
    assert.ok(row.briefing?.width > 0, where);
  }
  const ready = results.filter((row) => row.scenario === "home-ready");
  for (const row of ready) {
    assert.equal(row.looseScrollers.length, 1, row.size);
    assert.equal(row.hasCurrencyTable, true, row.size);
    assert.ok(row.holdings.y >= row.summary.y + row.summary.height, row.size);
    assert.equal(row.holdings.x, row.summary.x, row.size);
    const leftGutter = row.summary.x - row.area.x;
    const last = row.size === "qhd" ? row.signals : row.briefing;
    const rightGutter = row.area.x + row.area.width - last.x - last.width;
    assert.ok(leftGutter >= 0 && leftGutter <= 25, row.size);
    assert.ok(rightGutter >= 0 && rightGutter <= 25, row.size);
    if (row.size === "qhd" || row.size === "fullhd") {
      assert.equal(row.summary.y, row.briefing.y, row.size);
      assert.ok(row.briefing.x >= row.summary.x + row.summary.width, row.size);
      assert.ok(row.prose.width >= 700 && row.prose.width <= 721, row.size);
      if (row.size === "qhd") {
        assert.equal(row.signals.y, row.summary.y, row.size);
        assert.ok(
          row.signals.x >= row.briefing.x + row.briefing.width,
          row.size,
        );
      }
    } else if (row.size === "wide") {
      assert.equal(row.holdings.y, row.briefing.y, row.size);
      assert.ok(
        row.summary.width >= row.briefing.width + row.holdings.width,
        row.size,
      );
      assert.ok(row.summary.height < 400 && row.prose.width >= 480, row.size);
    } else {
      assert.equal(row.summary.x, row.briefing.x, row.size);
      assert.equal(row.summary.width, row.briefing.width, row.size);
      assert.ok(
        row.briefing.y >= row.holdings.y + row.holdings.height,
        row.size,
      );
    }
    const [assessment, candidates, economy] = row.reports;
    assert.ok(
      assessment.y + assessment.height <= candidates.y &&
        candidates.y + candidates.height <= economy.y,
      row.size,
    );
    assert.equal(assessment.x, economy.x, row.size);
    if (row.size === "qhd") {
      assert.equal(assessment.x, row.briefing.x, row.size);
      assert.equal(assessment.width, row.briefing.width, row.size);
      assert.ok(assessment.y >= row.briefing.y + row.briefing.height, row.size);
    }
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
      await holdings
        .getByRole("button", { name: "근거 자료 1건" })
        .press("Enter");
      await holdings.getByText("예시전자 분기보고서").waitFor();
      await page
        .locator("#candidates")
        .getByText("아직 조사 결과가 없어요.")
        .waitFor();
      const economy = page.locator("#economy");
      const updating = economy.getByText("새 자료를 확인하고 있어요.");
      await updating.waitFor();
      await economy
        .getByRole("button", { name: "근거 자료 1건" })
        .press("Enter");
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
