import test from "node:test";
import assert from "node:assert/strict";
import { ESLint } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
import { readFileSync } from "node:fs";
import { join } from "node:path";

test("DS06 숫자 변환은 세 계산 파일에만 허용된다", async () => {
  const engine = new ESLint({ cwd: desktopRoot });
  for (const [file, allowed] of [
    ["LineChart", true],
    ["DonutChart", true],
    ["Treemap", true],
    ["AreaChart", false],
    ["Card", false],
  ]) {
    const path = `src/shared/ds/${file}.tsx`;
    const input =
      readFileSync(join(desktopRoot, path), "utf8") +
      '\nconst probe = Number("1");\n';
    const [result] = await engine.lintText(input, { filePath: path });
    assert.equal(
      result.messages.some((m) => m.ruleId === "guard/no-number-conversion"),
      !allowed,
      file,
    );
  }
  const input =
    readFileSync(join(desktopRoot, "src/app/main.tsx"), "utf8") +
    '\nconst probe=Number("1");\n';
  const [result] = await engine.lintText(input, {
    filePath: "src/app/main.tsx",
  });
  assert.ok(
    result.messages.some((m) => m.ruleId === "guard/no-number-conversion"),
  );
});

test("DS06 기하 예외는 ChartFrame의 경로와 viewBox만이다", async () => {
  const engine = new ESLint({ cwd: desktopRoot });
  const path = "src/shared/ds/ChartFrame.tsx";
  const source = readFileSync(join(desktopRoot, path), "utf8");
  const [clean] = await engine.lintText(source, { filePath: path });
  assert.equal(clean.errorCount, 0, JSON.stringify(clean.messages));
  for (const [from, to, rule] of [
    ['viewBox="0 0 100 60"', "width={999}", "ds/no-presentation-props"],
    [
      "fill-chart-1",
      "fill-red-500",
      "better-tailwindcss/no-restricted-classes",
    ],
  ]) {
    const [bad] = await engine.lintText(source.replace(from, to), {
      filePath: path,
    });
    assert.ok(
      bad.messages.some((m) => m.ruleId === rule),
      to,
    );
  }
  const [outside] = await engine.lintText(
    'export const Probe=()=> <svg viewBox="0 0 100 60"/>;',
    { filePath: "src/shared/ds/AreaChart.tsx" },
  );
  assert.ok(
    outside.messages.some((m) => m.ruleId === "ds/no-presentation-props"),
  );
});
