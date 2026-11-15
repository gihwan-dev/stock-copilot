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
    ['viewBox="-1 -1 102 62"', "width={999}", "ds/no-presentation-props"],
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

test("DS06 원문 직접 표시는 모든 화면과 차트 파일에서 금지된다", async () => {
  const engine = new ESLint({ cwd: desktopRoot });
  for (const [path, allowed] of [
    ["src/shared/ds/ChartFrame.tsx", false],
    ["src/shared/ds/ChartTable.tsx", false],
    ["src/shared/ds/AreaChart.tsx", false],
    ["src/app/main.tsx", false],
  ]) {
    const source =
      readFileSync(join(desktopRoot, path), "utf8") +
      '\nimport {parseChartDecimal} from "' +
      (path.startsWith("src/app") ? "../shared/lib" : "../lib") +
      '";\nconst probeDecimal=parseChartDecimal("1");\nexport const DecimalProbe=():import("react").ReactElement|null=>probeDecimal.kind==="ok"?<>{probeDecimal.value}</>:null;\n';
    const [result] = await engine.lintText(source, { filePath: path });
    assert.equal(
      result.messages.some((m) => m.ruleId === "local/no-ui-text"),
      !allowed,
      path,
    );
  }
});

test("DS06 숨은 표와 SVG의 비례·포인터 예외는 승인 파일만 허용한다", async () => {
  const engine = new ESLint({ cwd: desktopRoot });
  for (const [path, allowed] of [
    ["src/shared/ds/ChartTable.tsx", true],
    ["src/shared/ds/AreaChart.tsx", false],
  ]) {
    const [result] = await engine.lintText(
      'const Probe=()=> <table className="sr-only"/>;',
      { filePath: path },
    );
    assert.equal(
      result.messages.some(
        (m) => m.ruleId === "better-tailwindcss/no-restricted-classes",
      ),
      !allowed,
      path,
    );
  }
  const path = "src/shared/ds/ChartFrame.tsx";
  const source = readFileSync(join(desktopRoot, path), "utf8");
  for (const [from, to] of [
    ['vectorEffect="non-scaling-stroke"', 'vectorEffect="arbitrary"'],
    ['pointerEvents="all"', 'pointerEvents="arbitrary"'],
  ]) {
    const [result] = await engine.lintText(source.replace(from, to), {
      filePath: path,
    });
    assert.ok(
      result.messages.some((m) => m.ruleId === "ds/no-presentation-props"),
      to,
    );
  }
});
