import assert from "node:assert/strict";
import test from "node:test";
import { withAuditServer } from "../../harness/screen-audit/run.mjs";

const nativeKey = "stock-copilot.dashboard-dock.v2.summary-row";
const legacyKey = "stock-copilot.dashboard-dock.v1.summary-row";
const tab = (page, panel) => page.locator(`#flexlayout-tabbutton-${panel}`);
const group = (page, panel) =>
  page.locator(".flexlayout__tabset").filter({ has: tab(page, panel) });
const float = (page, panel) =>
  page.locator(".flexlayout__float_window").filter({ has: tab(page, panel) });
const menu = async (page, panel, action) => {
  await tab(page, panel).click({ button: "right" });
  await page.getByRole("menuitem", { name: action, exact: true }).click();
};
const frame = (id, tabs, selectedId) => ({
  type: "tabset",
  id,
  tabs: tabs.map((id) => ({ id, data: id })),
  selectedId,
});
const legacy = {
  version: 1,
  root: {
    type: "split",
    id: "home-layout",
    orientation: "horizontal",
    children: [
      { ...frame("first", ["positions", "overview"], "positions"), size: 70 },
      { ...frame("second", ["signals"], "signals"), size: 30 },
    ],
  },
  floating: [
    {
      ...frame("research", ["economy", "candidates"], "candidates"),
      x: 100,
      y: 130,
      width: 410,
      height: 350,
    },
  ],
};
const readSaved = (page) =>
  page.evaluate((key) => {
    const params = new URLSearchParams(localStorage.getItem(key));
    return Object.fromEntries(
      [...params].map(([name, raw]) => [name, JSON.parse(raw)]),
    );
  }, nativeKey);

test("이전 배치의 분할·탭·부동 창을 보존하고 기본 도킹 동작과 복원을 다시 불러온다", async () => {
  await withAuditServer(async ({ browser, url }) => {
    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      locale: "ko-KR",
    });
    try {
      const page = await context.newPage();
      page.setDefaultTimeout(6000);
      const errors = [];
      page.on("pageerror", (e) => errors.push(e.message));
      await page.goto(url + "?s=home-ready");
      await tab(page, "overview").waitFor();
      await page.evaluate(
        ({ key, layout, native }) => {
          const params = new URLSearchParams();
          for (const [name, value] of Object.entries(layout))
            params.set(name, JSON.stringify(value));
          localStorage.setItem(key, "?" + params);
          const conflicting = {
            type: "row",
            id: "repeated",
            children: [
              {
                type: "tabset",
                id: "repeated",
                children: [{ type: "tab", id: "briefing" }],
              },
            ],
          };
          localStorage.setItem(
            native,
            "?" + new URLSearchParams({ layout: JSON.stringify(conflicting) }),
          );
        },
        { key: legacyKey, layout: legacy, native: nativeKey },
      );
      await page.reload();
      await float(page, "candidates").waitFor();
      assert.equal(
        await page.locator('[data-dock-workspace] [role="tab"]').count(),
        5,
      );
      assert.equal(
        await tab(page, "positions").getAttribute("aria-selected"),
        "true",
      );
      assert.equal(
        await tab(page, "candidates").getAttribute("aria-selected"),
        "true",
      );
      const a = await group(page, "positions").boundingBox();
      const b = await group(page, "signals").boundingBox();
      assert.ok(
        a.width / b.width > 2.1 && a.width / b.width < 2.6,
        `70:30 분할 ${a.width}/${b.width}`,
      );
      await tab(page, "overview").dragTo(
        page.locator("#flexlayout-tab-signals"),
        { steps: 12 },
      );
      await group(page, "signals")
        .locator("#flexlayout-tabbutton-overview")
        .waitFor();
      await menu(page, "overview", "탭 고정");
      await page.reload();
      await tab(page, "overview").waitFor();
      assert.match(
        await tab(page, "overview").getAttribute("aria-label"),
        /고정됨/u,
      );
      await menu(page, "overview", "탭 고정 해제");
      await menu(page, "overview", "새 그룹에 추가");
      await page.reload();
      await tab(page, "overview").waitFor();
      assert.match(JSON.stringify(await readSaved(page)), /"type":"tabgroup"/u);
      await menu(page, "positions", "독립 프레임으로 띄우기");
      await float(page, "positions").waitFor();
      await page.reload();
      await float(page, "positions").waitFor();
      const saved = await readSaved(page);
      const research = Object.values(saved.subLayouts).find((s) =>
        JSON.stringify(s.layout).includes("candidates"),
      );
      assert.deepEqual(research.rect, {
        x: 100,
        y: 130,
        width: 410,
        height: 350,
      });
      await tab(page, "signals").click();
      const target = page.locator("#flexlayout-tab-signals");
      const area = await target.boundingBox();
      await float(page, "positions")
        .getByLabel("프레임에 고정")
        .dragTo(target, {
          steps: 16,
          targetPosition: { x: area.width * 0.77, y: area.height * 0.56 },
        });
      await float(page, "positions").waitFor({ state: "detached" });
      await tab(page, "positions").waitFor();
      assert.match(
        JSON.stringify((await readSaved(page)).layout),
        /"id":"positions"/u,
      );
      await menu(page, "positions", "위젯 제거");
      await tab(page, "positions").waitFor({ state: "detached" });
      await page
        .getByRole("button", { name: "위젯 추가", exact: true })
        .click();
      await page
        .getByRole("button", { name: "보유 종목", exact: true })
        .click();
      await tab(page, "positions").waitFor();
      await page
        .getByRole("button", { name: "위젯 추가", exact: true })
        .click();
      await page
        .getByRole("button", { name: "기본 배치 복원", exact: true })
        .click();
      await tab(page, "briefing").waitFor();
      await page.reload();
      await tab(page, "briefing").waitFor();
      assert.equal(
        await page.locator('[data-dock-workspace] [role="tab"]').count(),
        8,
      );
      assert.equal(await page.locator(".flexlayout__float_window").count(), 0);
      assert.deepEqual(errors, []);
    } finally {
      await context.close();
    }
  });
});
