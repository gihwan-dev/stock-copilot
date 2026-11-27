import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { deterministicChecks } from "./run.mjs";

const { scenarios } = JSON.parse(readFileSync(new URL("./scenarios.json", import.meta.url)));
const samples = [
  ["kr", "한국 시장은 향후 한 달 상승 쪽에 무게를 둡니다. 코스피와 KODEX 200의 최근 흐름이 함께 올랐습니다."],
  ["us", "미국 시장은 다음 2주 하방 우위로 봅니다. SPY 일봉의 하락세가 근거입니다."],
  ["mixed", "한국은 한 달 약세 전망입니다. 최근 반등과 수출 개선은 반대 근거입니다."],
  ["recovered", "한국 시장은 한 달간 약세 쪽으로 기웁니다. 25일 발표된 수출 주문 감소가 근거입니다."],
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
    if (["recovered", "empty"].includes(key)) requests.push(["request_web_research", { mode: "user_message" }]);
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
      if (key === "recovered") assert.match(calls.at(-1).result.summary.untrusted_text, /11월 25일.*수출 주문이 감소/);
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
