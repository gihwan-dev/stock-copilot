import assert from "node:assert/strict";
import test from "node:test";
import { withAuditServer } from "../../harness/screen-audit/run.mjs";

const vertical =
  '[data-dock-workspace] [role="separator"][aria-orientation="vertical"]';
const ghost = ".flexlayout__splitter_drag";
const sizes = (page) =>
  page.locator("[data-widget-frame]").evaluateAll((frames) =>
    frames.map((frame) => {
      const { x, y, width, height } = frame.getBoundingClientRect();
      return [frame.dataset.widgetFrame, x, y, width, height];
    }),
  );
const moves = (page) => page.evaluate(() => globalThis.dockResizeWrites);
const closeTo = (actual, expected) =>
  assert.ok(Math.abs(actual - expected) < 2, `${actual} ≈ ${expected}`);

test("분할선을 놓을 때만 위젯 크기와 저장값이 바뀌고 취소·키보드 조절을 지원한다", async () => {
  await withAuditServer(async ({ browser, url }) => {
    const page = await browser.newPage({
      viewport: { width: 1440, height: 900 },
      colorScheme: "light",
    });
    try {
      await page.addInitScript(() => {
        const write = Storage.prototype.setItem;
        globalThis.dockResizeWrites = 0;
        Storage.prototype.setItem = function (key, value) {
          if (key.startsWith("stock-copilot.dashboard-dock.v2."))
            globalThis.dockResizeWrites++;
          return write.call(this, key, value);
        };
      });
      await page.goto(url + "?s=home-ready");
      const splitter = page.locator(vertical).first();
      await splitter.waitFor();
      await page.waitForFunction(() => globalThis.auditState === "ready");
      await page.evaluate(() => globalThis.document.fonts.ready);
      const initial = await splitter.boundingBox();
      const x = initial.x + initial.width / 2;
      const y = initial.y + initial.height / 2;
      const original = await sizes(page);
      const previousWrites = await moves(page);
      await page.mouse.move(x, y);
      await page.mouse.down();
      for (const delta of [40, 96]) {
        await page.mouse.move(x + delta, y, { steps: 12 });
        closeTo((await page.locator(ghost).boundingBox()).x, initial.x + delta);
        assert.deepEqual(await sizes(page), original);
        assert.equal(await moves(page), previousWrites);
      }
      await page.mouse.up();
      await page.locator(ghost).waitFor({ state: "detached" });
      closeTo((await splitter.boundingBox()).x, initial.x + 96);
      assert.notDeepEqual(await sizes(page), original);
      assert.equal(await moves(page), previousWrites + 1);
      const saved = await sizes(page);
      await page.mouse.down();
      await page.mouse.move(x + 130, y, { steps: 8 });
      await page.locator(ghost).waitFor();
      await splitter.dispatchEvent("pointercancel", { pointerType: "mouse" });
      await page.locator(ghost).waitFor({ state: "detached" });
      await page.mouse.up();
      assert.deepEqual(await sizes(page), saved);
      assert.equal(await moves(page), previousWrites + 1);
      await splitter.press("ArrowRight");
      closeTo((await splitter.boundingBox()).x, initial.x + 106);
      assert.equal(await moves(page), previousWrites + 2);
      await page.reload();
      await splitter.waitFor();
      closeTo((await splitter.boundingBox()).x, initial.x + 106);
    } finally {
      await page.close();
    }
  });
});

test("위젯 간격과 모서리·본문 여백에 설정값을 쓰고 두 테마에서 손잡이를 표시한다", async () => {
  await withAuditServer(async ({ browser, url }) => {
    const page = await browser.newPage({
      viewport: { width: 1440, height: 900 },
      colorScheme: "light",
    });
    try {
      await page.goto(url + "?s=home-ready");
      const splitter = page.locator(vertical).first();
      const grip = splitter.locator(".flexlayout__splitter_handle");
      await grip.waitFor();
      const gripColor = () =>
        grip.evaluate((el) => getComputedStyle(el).backgroundColor);
      const light = await gripColor();
      await page.emulateMedia({ colorScheme: "dark" });
      await page.waitForFunction(
        (prior) =>
          getComputedStyle(
            document.querySelector(".flexlayout__splitter_handle"),
          ).backgroundColor !== prior,
        light,
      );
      assert.equal(
        await grip.evaluate((el) => getComputedStyle(el).visibility),
        "visible",
      );
      for (const setting of [0, 4, 8, 12]) {
        const padding = setting === 12 ? 16 : setting;
        await page.evaluate(
          ({ setting, padding }) => {
            localStorage.setItem(
              "stock-copilot.appearance.gap",
              String(setting),
            );
            localStorage.setItem(
              "stock-copilot.appearance.radius",
              String(setting),
            );
            localStorage.setItem(
              "stock-copilot.appearance.padding",
              String(padding),
            );
          },
          { setting, padding },
        );
        await page.reload();
        await page
          .locator(
            `[data-widget-gap="${setting}"][data-widget-padding="${padding}"]`,
          )
          .waitFor();
        await splitter.waitFor();
        const metrics = await splitter.evaluate((el) => ({
          width: el.getBoundingClientRect().width,
          target: parseFloat(getComputedStyle(el, "::before").width),
          radius: parseFloat(
            getComputedStyle(document.querySelector(".flexlayout__tabset"))
              .borderRadius,
          ),
          padding: parseFloat(
            getComputedStyle(document.querySelector("[data-widget-frame]"))
              .paddingLeft,
          ),
        }));
        assert.deepEqual(metrics, {
          width: setting || 1,
          target: 12,
          radius: setting,
          padding,
        });
      }
    } finally {
      await page.close();
    }
  });
});
