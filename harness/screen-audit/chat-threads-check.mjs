import assert from "node:assert/strict";
import { withAuditServer, inspectPage } from "./run.mjs";

await withAuditServer(async ({ browser, url }) => {
  for (const viewport of [
    { width: 1440, height: 900 },
    { width: 960, height: 640 },
  ]) {
    const page = await browser.newPage({ viewport });
    try {
      await page.goto(url + "?s=chat-threads");
      await page.waitForFunction(() => globalThis.auditState === "ready");
      const nav = page.getByRole("navigation", { name: "채팅", exact: true });
      const row = (name) => nav.getByRole("button", { name, exact: true });
      const box = page.getByRole("textbox", { name: "메시지" });
      const toggle = row("채팅");
      const fresh = row("새 채팅");
      await toggle.focus();
      await toggle.press("Space");
      assert.equal(await toggle.getAttribute("aria-expanded"), "false");
      await toggle.press("Enter");
      await row("기존 시장 대화").waitFor();
      await fresh.click();
      await row("제목 없는 대화").waitFor();
      const long = "테스트용긴제목".repeat(20);
      await box.fill(long);
      await box.press("Enter");
      await row(long).waitFor();
      await fresh.click();
      await row("제목 없는 대화").waitFor();
      await box.fill("두 번째 대화");
      await box.press("Enter");
      await row("두 번째 대화").waitFor();
      await box.fill("설정 왕복 초안");
      await page
        .getByRole("button", { name: "설정", exact: true })
        .first()
        .click();
      await nav.waitFor({ state: "hidden" });
      await page
        .getByRole("button", { name: "이전 화면으로", exact: true })
        .first()
        .click();
      await box.waitFor();
      assert.equal(await box.inputValue(), "설정 왕복 초안");
      await row(long).click();
      await page.getByText("예시 응답 · " + long, { exact: true }).waitFor();
      assert.equal(await box.inputValue(), "");
      assert.equal(
        await page
          .getByText("예시 응답 · 두 번째 대화", { exact: true })
          .count(),
        0,
      );
      assert.deepEqual(await inspectPage(page), []);
      const bounds = await box.boundingBox();
      assert.ok(
        bounds &&
          bounds.width > 200 &&
          bounds.y + bounds.height <= viewport.height,
      );
      await row("기존 시장 대화").focus();
      await row("기존 시장 대화").press("Enter");
      await page
        .getByText("저장된 예시 답변입니다.", { exact: true })
        .waitFor();
      for (let index = 0; index < 12; index += 1) {
        const count = await nav.getByRole("listitem").count();
        await fresh.click();
        await page.waitForFunction(
          (expected) =>
            document.querySelectorAll('nav[aria-label="채팅"] li').length ===
            expected,
          count + 1,
        );
      }
      const treeBounds = await nav.boundingBox();
      const inputBounds = await box.boundingBox();
      assert.ok(
        treeBounds &&
          inputBounds &&
          treeBounds.x + treeBounds.width <= inputBounds.x + 1,
      );
      assert.ok(
        inputBounds.width > 200 &&
          inputBounds.y + inputBounds.height <= viewport.height,
      );
      assert.deepEqual(await inspectPage(page), []);
      console.log(`chat-threads ${viewport.width}x${viewport.height} passed`);
    } finally {
      await page.close();
    }
  }
});
