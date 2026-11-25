import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { mkdtempSync } from "../../../harness/ci/test-temp.mjs";
import { inspectNumberWidths } from "../../../harness/screen-audit/number-width.mjs";
import {
  launchBrowser,
  sizes,
  withAuditServer,
} from "../../../harness/screen-audit/run.mjs";
import { repoRoot } from "./paths.mjs";

const scenarios = [
  "home-ready",
  "transactions-ready",
  "status-ready",
  "instrument-detail",
];
const check = (page) =>
  page.evaluate(`(${inspectNumberWidths.toString()})(globalThis.document)`);

test("number widths: a proportional fallback font and a page without the bundled font are caught", async () => {
  const browser = await launchBrowser();
  try {
    const tab = await browser.newPage({ viewport: sizes.minimum });
    await tab.setContent(
      `<!doctype html><html><body><span class="tabular-nums" style="font-family:'Apple SD Gothic Neo',serif;font-size:14px">1,234,567원</span></body></html>`,
    );
    const kinds = (await check(tab)).findings.map((finding) => finding.kind);
    assert.ok(kinds.includes("proportional-digits"), JSON.stringify(kinds));
    assert.ok(kinds.includes("font-not-loaded"), JSON.stringify(kinds));
  } finally {
    await browser.close();
  }
});

test("number widths: every number in the built app is tabular in Chromium and in the system WKWebView", async () => {
  await withAuditServer(async ({ browser, url }) => {
    for (const scenario of scenarios)
      for (const theme of ["light", "dark"]) {
        const context = await browser.newContext({
          viewport: sizes.wide,
          colorScheme: theme,
        });
        const page = await context.newPage();
        await page.goto(`${url}?s=${scenario}`);
        await page.waitForFunction(() => globalThis.auditState === "ready");
        await page.evaluate(() => globalThis.document.fonts.ready);
        const result = await check(page);
        await context.close();
        assert.ok(result.count > 0, `${scenario}: 숫자 없음`);
        assert.deepEqual(result.findings, [], `${scenario} ${theme}`);
      }
    const folder = mkdtempSync(join(repoRoot, "target/screen-audit-wk-"));
    const checker = join(folder, "checker.js");
    writeFileSync(checker, inspectNumberWidths.toString());
    const run = await new Promise((done) => {
      const child = spawn("swift", [
        join(repoRoot, "harness/screen-audit/wk-numbers.swift"),
        checker,
        ...scenarios.map((scenario) => `${url}?s=${scenario}`),
      ]);
      const out = { stdout: "", stderr: "" };
      child.stdout.on("data", (chunk) => {
        out.stdout += chunk;
      });
      child.stderr.on("data", (chunk) => {
        out.stderr += chunk;
      });
      child.on("close", (status) => done({ ...out, status }));
    });
    assert.equal(run.status, 0, run.stderr);
    const rows = JSON.parse(run.stdout);
    assert.equal(rows.length, scenarios.length);
    for (const row of rows) {
      assert.equal(row.state, "ready", JSON.stringify(row));
      assert.ok(row.result.count > 0, JSON.stringify(row));
      assert.deepEqual(row.result.findings, [], JSON.stringify(row));
    }
  });
});
