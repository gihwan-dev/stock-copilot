import assert from "node:assert/strict";
import { test } from "node:test";
import { withAuditServer } from "../../harness/screen-audit/run.mjs";

test("일봉을 키보드로 탐색해도 같은 캔버스의 크기 감시를 다시 등록하지 않는다", async () => {
  await withAuditServer(async ({ browser, url }) => {
    const page = await browser.newPage({
      viewport: { width: 1440, height: 900 },
    });
    try {
      await page.addInitScript(() => {
        const registrations = new WeakMap();
        globalThis.chartRegistrations = (canvas) =>
          registrations.get(canvas) ?? 0;
        globalThis.ResizeObserver = class extends ResizeObserver {
          observe(element, options) {
            if (element instanceof HTMLCanvasElement) {
              registrations.set(element, (registrations.get(element) ?? 0) + 1);
            }
            super.observe(element, options);
          }
        };
      });
      await page.goto(url + "?s=instrument-detail");
      await page.waitForFunction(() => globalThis.auditState === "ready");
      const chart = page.getByRole("figure", { name: "일봉 차트" });
      const explore = chart.getByRole("button", { name: /차트 항목 살펴보기/ });
      const count = () =>
        chart
          .locator("canvas")
          .evaluate((node) => globalThis.chartRegistrations(node));
      const initial = await count();
      assert.ok(initial > 0);
      await explore.focus();
      await explore.press("Home");
      const first = await chart.getByRole("status").innerText();
      await explore.press("End");
      const last = await chart.getByRole("status").innerText();
      assert.notEqual(first, last);
      for (let index = 0; index < 12; index += 1) {
        await explore.press("ArrowLeft");
      }
      assert.equal(await count(), initial);
    } finally {
      await page.close();
    }
  });
});
