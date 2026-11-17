import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { runExamples, withBrowser } from "./environment.mjs";

await withBrowser((context) => {
  runExamples(context);
  const { root, output, container, vitest, image, total } = context;
  const batches = [];
  const batchSize = 60;
  for (let first = 0; first < total; first += batchSize) {
    const reportPath = join(output, `catalog-${first}.json`);
    rmSync(reportPath, { force: true });
    const result = spawnSync(
      "docker",
      [
        ...container,
        "--env",
        `DS20_FIRST=${first}`,
        "--env",
        `DS20_SIZE=${batchSize}`,
        image.Id,
        ...vitest,
        "catalog.browser.ts",
        "--reporter=default",
        "--reporter=json",
        `--outputFile.json=/work/target/DS-20/catalog-${first}.json`,
      ],
      { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    );
    const log = `${result.stdout ?? ""}${result.stderr ?? ""}`.replace(
      new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, "gu"),
      "",
    );
    writeFileSync(join(output, `catalog-${first}.log`), log);
    const line = log
      .split("\n")
      .find((item) => item.includes("DS20_CATALOG_FINDINGS "));
    if (line === undefined)
      throw new Error(
        `이야기 측정 누락: ${first}, target/DS-20/catalog-${first}.log`,
      );
    const value = JSON.parse(
      line.slice(
        line.indexOf("DS20_CATALOG_FINDINGS ") +
          "DS20_CATALOG_FINDINGS ".length,
      ),
    );
    if (
      value.first !== first ||
      value.total !== total ||
      value.count !== Math.min(batchSize, total - first) ||
      value.errors.length !== 0
    )
      throw new Error(`이야기 측정 범위 불일치: ${first}`);
    if (
      !existsSync(reportPath) ||
      log.includes("Unhandled Error") ||
      log.includes("Failed Suites") ||
      log.includes("Vite unexpectedly reloaded")
    )
      throw new Error(`이야기 실행 자체가 불완전하다: ${first}`);
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    const assertions = report.testResults.flatMap(
      (suite) => suite.assertionResults,
    );
    const expectedStatus = value.results.length === 0 ? "passed" : "failed";
    if (
      report.numTotalTests !== 4 ||
      report.numPendingTests !== 0 ||
      report.numTodoTests !== 0 ||
      assertions.length !== 4 ||
      assertions[0].status !== "passed" ||
      assertions[1].status !== "passed" ||
      assertions[2].status !== expectedStatus ||
      assertions[3].status !== "passed"
    )
      throw new Error(`이야기 테스트 실행 결과 불완전: ${first}`);
    if (result.status !== (value.results.length === 0 ? 0 : 1))
      throw new Error(`이야기 측정 종료 상태 불일치: ${first}`);
    batches.push(value);
    process.stdout.write(
      `overflow: ${first + value.count}/${total}, 위반 조건 ${value.results.length}\n`,
    );
  }
  const findings = batches.flatMap((batch) => batch.results);
  writeFileSync(
    join(output, "catalog-findings.json"),
    `${JSON.stringify({ total, batches: batches.length, findings }, null, 2)}\n`,
  );
  process.stdout.write(
    `overflow: ${total} 이야기, 위반 조건 ${findings.length}\n`,
  );
  if (findings.length !== 0) process.exitCode = 1;
});
