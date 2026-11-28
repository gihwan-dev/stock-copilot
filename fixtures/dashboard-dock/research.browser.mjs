import assert from "node:assert/strict";
import test from "node:test";
import { createRequire } from "node:module";
import { withAuditServer } from "../../harness/screen-audit/run.mjs";

const require = createRequire(
  new URL("../../apps/desktop/package.json", import.meta.url),
);
const { expect } = require("@playwright/test");

const tabs = {
  briefing: "오늘 브리핑",
  positions: "보유 종목",
  holdings: "보유 자산 평가",
  candidates: "새 종목 후보",
  economy: "경제와 산업 동향",
};

const tab = (page, widget) =>
  page.getByRole("tab", { name: tabs[widget], exact: true });

const activate = async (page, widget) => {
  const target = tab(page, widget);
  await target.press("Enter");
  await expect(target).toHaveAttribute("aria-selected", "true");
};

const report = (page, widget) => page.locator("#" + widget);
const evidence = (scope) =>
  scope.getByRole("button", { name: "근거 자료 1건" });
const open = async ({ browser, url }, scenario, fakeTime = false) => {
  const page = await browser.newPage({ viewport: { width: 960, height: 640 } });
  page.setDefaultTimeout(5000);
  if (fakeTime) await page.clock.install();
  await page.goto(url + "?s=" + scenario);
  await page.waitForFunction(() => globalThis.auditState === "ready");
  await page.locator("[data-dock-workspace]").waitFor();
  return page;
};

test("도킹 대시보드 조회 흐름", async (t) => {
  await withAuditServer(async (target) => {
    await t.test(
      "키보드로 브리핑과 조사 근거, 마지막 문단, 원문에 접근한다",
      async () => {
        const page = await open(target, "home-ready");
        try {
          await activate(page, "briefing");
          const briefing = report(page, "daily-briefing");
          const briefingSources = briefing.getByRole("button", {
            name: "브리핑 근거 자료",
          });
          await expect(briefingSources).toHaveAttribute(
            "aria-expanded",
            "false",
          );
          await briefingSources.press("Enter");
          await expect(briefingSources).toHaveAttribute(
            "aria-expanded",
            "true",
          );
          await briefingSources.press("Enter");
          await expect(
            briefing.getByText("만든 시각", { exact: true }),
          ).toBeVisible();

          await activate(page, "holdings");
          const holdings = report(page, "holdings");
          await expect(
            holdings.getByText("005930", { exact: true }),
          ).toBeVisible();
          await expect(evidence(holdings)).toHaveAttribute(
            "aria-expanded",
            "false",
          );
          await holdings
            .getByRole("link", { name: "[1]", exact: true })
            .press("Enter");
          await expect(holdings.locator("#holdings-1")).toBeFocused();
          await expect(evidence(holdings)).toHaveAttribute(
            "aria-expanded",
            "true",
          );
          await expect(
            holdings.getByRole("link", { name: /원문 열기/u }),
          ).toHaveCount(0);

          await activate(page, "economy");
          const economy = report(page, "economy");
          const last = economy.getByText(
            "설비 투자가 큰 제조업에서는 이자 비용과 신규 수주를 함께 살펴야 합니다.",
            { exact: true },
          );
          await last.scrollIntoViewIfNeeded();
          await expect(last).toBeInViewport();

          await page.evaluate(() => {
            const ipc = globalThis.__TAURI_INTERNALS__;
            const invoke = ipc.invoke.bind(ipc);
            globalThis.observedOriginals = [];
            ipc.invoke = (command, ...args) => {
              if (command === "open_external")
                globalThis.observedOriginals.push(args[0].request.url);
              return invoke(command, ...args);
            };
          });
          await evidence(economy).press("Enter");
          const original = economy.getByRole("link", { name: /원문 열기/u });
          await original.press("Enter");
          await expect
            .poll(() => page.evaluate(() => globalThis.observedOriginals))
            .toEqual(["https://example.org/report"]);
        } finally {
          await page.close();
        }
      },
    );

    await t.test(
      "실 해시와 설정 왕복·새로고침 뒤 후보 상세를 열고 복귀한다",
      async () => {
        const page = await open(target, "home-ready&hash-route#/");
        try {
          await activate(page, "candidates");
          assert.equal(new URL(page.url()).hash, "#/");
          await page
            .getByRole("button", { name: "설정", exact: true })
            .press("Enter");
          await expect(page).toHaveURL(/#\/settings$/u);
          await page.goBack();
          await expect(tab(page, "candidates")).toBeVisible();
          assert.equal(new URL(page.url()).hash, "#/");
          await page.reload();
          await expect(tab(page, "candidates")).toBeVisible();
          assert.equal(new URL(page.url()).hash, "#/");

          await activate(page, "candidates");
          const details = report(page, "candidates").getByRole("button", {
            name: "종목 상세",
          });
          await details.press("Enter");
          const close = page.getByRole("button", { name: "닫기", exact: true });
          await expect(close).toBeVisible();
          const quote = page.locator("[data-ui-card]").filter({
            has: page.getByRole("heading", { name: "시세", exact: true }),
          });
          await expect(
            quote.getByText("예시전력", { exact: true }),
          ).toBeVisible();
          await expect(
            quote.getByText("012345", { exact: true }),
          ).toBeVisible();
          await close.press("Enter");
          await expect(details).toBeVisible();
          await activate(page, "positions");
          await page
            .getByRole("button", { name: "예시전자", exact: true })
            .press("Enter");
          await expect(close).toBeVisible();
          await expect(
            quote.getByText("예시전자", { exact: true }),
          ).toBeVisible();
          await expect(
            quote.getByText("005930", { exact: true }),
          ).toBeVisible();
        } finally {
          await page.close();
        }
      },
    );

    await t.test(
      "부분 실패·빈 결과·갱신을 표시하고 TTL 이후 복구한다",
      async () => {
        const page = await open(target, "home-research-refresh", true);
        try {
          await activate(page, "holdings");
          const holdings = report(page, "holdings");
          const failed = holdings.getByText(
            "이번 조사 결과를 불러오지 못했어요.",
          );
          await expect(failed).toBeVisible();
          await evidence(holdings).press("Enter");
          await expect(holdings.getByText("예시전자 분기보고서")).toBeVisible();

          await activate(page, "candidates");
          const candidates = report(page, "candidates");
          await expect(
            candidates.getByText("아직 조사 결과가 없어요."),
          ).toBeVisible();

          await activate(page, "economy");
          const economy = report(page, "economy");
          const updating = economy.getByText("새 자료를 확인하고 있어요.");
          await expect(updating).toBeVisible();
          await evidence(economy).press("Enter");
          await page.clock.fastForward(5100);
          await expect(updating).toHaveCount(0);
          await expect(economy.getByText("산업 동향 조사")).toBeVisible();
          await page.clock.fastForward(300_100);
          await activate(page, "candidates");
          await expect(
            candidates.getByText("012345", { exact: true }),
          ).toBeVisible();
          await expect(
            candidates.getByRole("button", { name: "종목 상세" }),
          ).toHaveCount(0);
          await activate(page, "holdings");
          await expect(failed).toHaveCount(0);
        } finally {
          await page.close();
        }
      },
    );
  });
});
