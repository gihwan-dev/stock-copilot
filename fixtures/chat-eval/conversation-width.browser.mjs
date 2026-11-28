import assert from "node:assert/strict";
import { withAuditServer } from "../../harness/screen-audit/run.mjs";

await withAuditServer(async ({ browser, url }) => {
  for (const viewport of [
    { width: 960, height: 640 },
    { width: 1440, height: 900 },
  ]) {
    const page = await browser.newPage({ viewport });
    try {
      await page.goto(url + "?s=chat-threads");
      const nav = page.getByRole("navigation", { name: "채팅", exact: true });
      const toggle = nav.getByRole("button", { name: "채팅", exact: true });
      const handle = page.getByRole("separator", { name: "채팅", exact: true });
      const box = page.getByRole("textbox", { name: "메시지" });
      await nav
        .getByRole("button", { name: "기존 시장 대화", exact: true })
        .click();
      await box.fill("목록 크기 조절 초안");
      const width = async () => (await nav.boundingBox()).width;
      const initial = await width();
      const bounds = await handle.boundingBox();
      const x = bounds.x + bounds.width / 2;
      const y = bounds.y + bounds.height / 2;
      await page.mouse.move(x, y);
      await page.mouse.down();
      await page.mouse.move(x + 48, y, { steps: 5 });
      await page.mouse.up();
      const dragged = await width();
      assert.ok(dragged > initial + 30);
      await handle.press("ArrowLeft");
      const resized = await width();
      assert.ok(resized < dragged);
      const before = await box.boundingBox();
      await toggle.click();
      assert.equal(await toggle.getAttribute("aria-expanded"), "false");
      assert.ok((await box.boundingBox()).width > before.width + 30);
      await toggle.press("Enter");
      assert.ok(Math.abs((await width()) - resized) <= 1);
      assert.equal(
        await nav.locator('[aria-current="page"]').innerText(),
        "기존 시장 대화",
      );
      assert.equal(await box.inputValue(), "목록 크기 조절 초안");
      console.log(viewport.width, { initial, dragged, resized });
    } finally {
      await page.close();
    }
  }
});
