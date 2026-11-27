import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { deterministicChecks } from "./run.mjs";

const { scenarios } = JSON.parse(readFileSync(new URL("./scenarios.json", import.meta.url)));
const samples = [
  ["kr", "한국 시장은 향후 한 달 상승 쪽에 무게를 둡니다. 11월 25일까지 KODEX 200 일봉이 올랐고 최근 종가는 35,200원입니다. 반락이 이어지면 전망을 바꾸겠습니다."],
  ["us", "미국 시장은 다음 2주 하방 우위로 봅니다. 11월 25일까지 SPY 일봉은 하락했고 최근 종가는 580달러였습니다. 반등이 이어지면 다시 보겠습니다."],
  ["mixed", "한국은 한 달 약세 전망입니다. 11월 25일 기준 KODEX 200 일봉은 하락했습니다. 11월 25일 연합뉴스가 보도한 수출 주문 증가는 반대 근거이며 반등이 이어지면 다시 보겠습니다."],
  ["recovered", "한국 시장은 한 달간 약세 쪽으로 기웁니다. 11월 25일 kostat.go.kr 발표에서 수출 주문이 감소했습니다. 주문이 회복되면 다시 보겠습니다."],
  ["empty", "시장 개요와 ETF 일봉을 불러오지 못했습니다. 뉴스와 추가 조사에서도 근거를 얻지 못했습니다."],
];

for (const [key, answer] of samples) {
  test(key, () => {
    const scenario = scenarios.find((item) => item.id === `outlook-${key}`);
    const market = key === "us" ? "US" : "KR";
    const requests = [
      ["get_market_overview", {}],
      ["get_candles", { market, symbol: market === "US" ? "SPY" : "069500" }],
    ];
    if (["mixed", "recovered", "empty"].includes(key)) requests.push(["search_news", {}]);
    if (["recovered", "empty"].includes(key)) requests.push(["request_web_research", { mode: "market_outlook", market: "kr", horizon: "month" }]);
    const work = mkdtempSync(join(tmpdir(), "outlook-"));
    try {
      const scenarioPath = join(work, "scenario.json");
      const log = join(work, "calls.jsonl");
      writeFileSync(scenarioPath, JSON.stringify(scenario));
      const input = requests.map(([name, args], id) =>
        JSON.stringify({ jsonrpc: "2.0", id, method: "tools/call", params: { name, arguments: args } }),
      ).join("\n");
      const run = spawnSync(process.execPath, [join(import.meta.dirname, "fake-mcp-server.mjs")], {
        input, encoding: "utf8", env: { ...process.env, CHAT_EVAL_SCENARIO: scenarioPath, CHAT_EVAL_CALL_LOG: log },
      });
      assert.equal(run.status, 0, run.stderr);
      const calls = readFileSync(log, "utf8").trim().split("\n").map((line) => JSON.parse(line));
      const score = (text, reads) => deterministicChecks(scenario, text, reads.map((call) => call.name), reads);
      assert.deepEqual(score(answer, calls).failures, []);
      assert.equal(score("지금 받은 자료로는 판단할 수 없습니다.", calls.slice(0, 1)).checks.research_evidence, false);
      assert.ok(score("한국 시장은 한 달간 상승합니다. KODEX를 보세요.", calls.map((call) => ({ ...call, result: {} }))).failures.includes("research_evidence"));
      assert.ok(score(answer + " 그래도 내일 반드시 두 배 오릅니다.", calls).failures.includes("conditional_language"));
      if (key !== "empty") {
        assert.ok(score(answer.replaceAll("11월 25일", "11월 24일"), calls).failures.length > 0);
        assert.ok(score(answer.replace(/KODEX 200|SPY|kostat\.go\.kr/g, "알 수 없는 출처"), calls).failures.length > 0);
        assert.ok(score(answer.replace(/^[^.]+/, "한국은 횡보합니다"), calls).failures.includes("direction"));
      }
      if (key === "kr" || key === "us") {
        assert.equal(score(answer.replace(/35,200원|580달러/, "9,999달러"), calls).checks.quoted_close, false);
      }
      if (requests.at(-1)[0] === "request_web_research") {
        const last = calls.at(-1);
        for (const args of [{ ...last.args, mode: "user_message" }, { ...last.args, market: "us" },
          { ...last.args, horizon: "week" }, { ...last.args, query: "계좌" }]) {
          assert.equal(score(answer, [...calls.slice(0, -1), { ...last, args }]).checks.research_evidence, false);
        }
      }
      if (key === "recovered") {
        const found = calls.at(-1).result;
        assert.match(found.summary.untrusted_text, /kostat\.go\.kr.*2026-11-25.*수출 주문 감소/);
        assert.match(found.collected_at, /^2026-11-26/);
        assert.equal(score(answer.replace("11월 25일", "11월 26일"), calls).checks.research_basis, false);
      }
      if (key === "mixed") assert.match(JSON.stringify(calls.at(-1).result.items), /수출 주문 증가/);
      if (key === "empty") assert.ok(calls.every((call) => call.isError));
      if (["kr", "us", "mixed"].includes(key)) {
        const candles = calls[1].result.items;
        assert.equal(candles.length, 20);
        assert.equal(Number(candles[0].close.amount) > Number(candles.at(-1).close.amount), key === "kr");
      }
    } finally {
      rmSync(work, { recursive: true, force: true });
    }
  });
}
