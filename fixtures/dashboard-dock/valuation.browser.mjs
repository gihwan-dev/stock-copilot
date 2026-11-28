import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { withAuditServer } from "../../harness/screen-audit/run.mjs";

const inspectTableBounds = (doc) =>
  [...doc.querySelectorAll("[data-widget-frame] table")]
    .filter((table) => table.offsetHeight > 0 && table.caption)
    .map((table) => {
      const lastRow = table.rows[table.rows.length - 1];
      const surface = table.closest("[data-widget-table]");
      return {
        caption: table.caption.innerText,
        overflow: Math.round(
          lastRow.getBoundingClientRect().bottom -
            surface.getBoundingClientRect().bottom,
        ),
      };
    });

const checkBounds = (tables) => {
  assert.ok(tables.length > 0, "표를 표시한 프레임");
  for (const table of tables) {
    assert.ok(table.overflow <= 1, JSON.stringify(table));
  }
};

test("평가 표의 기간별 상태와 자산배분의 마지막 행이 프레임에 맞는다", async (t) => {
  await withAuditServer(async ({ browser, url }) => {
    const route = url + "?s=home-ready";
    for (const viewport of [
      { width: 960, height: 640 },
      { width: 2560, height: 1440 },
    ]) {
      const page = await browser.newPage({ viewport, colorScheme: "dark" });
      try {
        await page.goto(route);
        const metrics = page.getByRole("table", {
          name: "손익과 수익률",
          exact: true,
        });
        const year = metrics
          .getByRole("row")
          .filter({
            has: page.getByRole("rowheader", { name: "1년", exact: true }),
          });
        await year.getByText("자료 부족", { exact: true }).waitFor();
        assert.deepEqual(
          await metrics.getByRole("rowheader").allTextContents(),
          ["오늘 손익", "1개월", "3개월", "6개월", "1년", "최대 낙폭(1년)"],
        );
        await page
          .getByRole("button", { name: "종목·섹터별 비중 보기" })
          .click();
        const allocation = page.locator("[data-widget-frame=allocation]");
        const note = allocation.getByText("보유 종목 평가액과 매수가능금액을", {
          exact: false,
        });
        const stocks = allocation.getByRole("table", {
          name: "종목별 비중",
          exact: true,
        });
        await stocks.getByRole("row").last().scrollIntoViewIfNeeded();
        const bottom = await stocks.getByRole("row").last().boundingBox();
        const below = await note.boundingBox();
        assert.ok(
          below.y >= bottom.y + bottom.height,
          "설명은 마지막 행 다음에 온다",
        );
        checkBounds(
          await page.evaluate(
            `(${inspectTableBounds.toString()})(globalThis.document)`,
          ),
        );
      } finally {
        await page.close();
      }
    }
    await t.test(
      "macOS WKWebView에서 캡션이 있는 표의 마지막 행이 표 영역 안에 있다",
      { skip: process.platform !== "darwin" },
      async () => {
        const directory = await mkdtemp(join(tmpdir(), "stock-widget-table-"));
        try {
          const checker = join(directory, "check.js");
          await writeFile(checker, inspectTableBounds.toString());
          const runner = fileURLToPath(
            new URL(
              "../../harness/screen-audit/wk-numbers.swift",
              import.meta.url,
            ),
          );
          const { stdout } = await promisify(execFile)(
            "swift",
            [runner, checker, route],
            { timeout: 120000 },
          );
          const [screen] = JSON.parse(stdout);
          assert.equal(screen.state, "ready");
          checkBounds(screen.result);
        } finally {
          await rm(directory, { recursive: true, force: true });
        }
      },
    );
  });
});
