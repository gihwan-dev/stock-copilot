import { appendFileSync, existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const directory = "target/mutants/mutants.out";
const read = (name) => readFileSync(join(directory, name), "utf8");
let state = "불완전";
try {
  const planned = JSON.parse(read("mutants.json"));
  if (
    Array.isArray(planned) &&
    planned.length === 0 &&
    !existsSync(join(directory, "outcomes.json")) &&
    ["missed.txt", "timeout.txt", "caught.txt", "unviable.txt"].every(
      (name) => existsSync(join(directory, name)) && read(name).trim() === "",
    )
  )
    state = "대상 없음";
  else {
    const outcomes = JSON.parse(read("outcomes.json"));
    if (
      Array.isArray(planned) &&
      planned.length > 0 &&
      outcomes.total_mutants === planned.length &&
      outcomes.outcomes.some(
        (entry) => entry.scenario === "Baseline" && entry.summary === "Success",
      )
    )
      state = "완주";
  }
} catch {
  state = "불완전";
}
const result = ["missed.txt", "timeout.txt", "caught.txt", "unviable.txt"]
  .map((name) => {
    const file = join(directory, name);
    const text = existsSync(file) ? read(name) : "결과 파일 없음";
    return `### ${name}\n\n\`\`\`text\n${text}\n\`\`\`\n`;
  })
  .join("\n");
const outcome = process.env.MUTATION_OUTCOME;
const summary = `## 밤 전체 변이 결과\n\n조각: ${process.env.MUTATION_SHARD ?? "알 수 없음"}\n\n실행: ${outcome ?? "알 수 없음"}, 결과: ${state}\n\n${result}`;
process.stdout.write(summary);
if (process.env.GITHUB_STEP_SUMMARY !== undefined) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary);
}
if (
  outcome !== undefined &&
  (outcome !== "success" || !["완주", "대상 없음"].includes(state))
) {
  process.exitCode = 1;
}
