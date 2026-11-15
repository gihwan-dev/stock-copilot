import { appendFileSync, existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const directory = "target/mutants/mutants.out";
const result = ["missed.txt", "timeout.txt", "caught.txt", "unviable.txt"]
  .map((name) => {
    const file = join(directory, name);
    const text = existsSync(file)
      ? readFileSync(file, "utf8")
      : "결과 파일 없음";
    return `### ${name}\n\n\`\`\`text\n${text}\n\`\`\`\n`;
  })
  .join("\n");
const summary = `## 밤 전체 변이 결과\n\n${result}`;
process.stdout.write(summary);
if (process.env.GITHUB_STEP_SUMMARY !== undefined) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary);
}
