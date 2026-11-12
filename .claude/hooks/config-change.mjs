import { readInput, writeOutput } from "./lib/io.mjs";

const input = readInput();

writeOutput({
  decision: "block",
  reason: `세션 중 설정 변경(${input.source ?? "알 수 없음"})은 이 세션에 적용하지 않는다 (docs/harness.md 7.3). 세션을 다시 시작하라.`,
});
