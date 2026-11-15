import test, { before, after } from "node:test";
import assert from "node:assert/strict";
import { componentsDom } from "./ds-components-dom.mjs";
import { desktopRoot } from "./paths.mjs";
import { join } from "node:path";

const source = `
import {createElement} from 'react';
import {render, fireEvent, cleanup} from '@testing-library/react';
import {LineChart,AreaChart,DonutChart,Treemap} from DS_ENTRY;
import {lineCoordinates} from '${join(desktopRoot, "src/shared/ds/LineChart.tsx")}';
import {donutArcs} from '${join(desktopRoot, "src/shared/ds/DonutChart.tsx")}';
import {treemapRectangles} from '${join(desktopRoot, "src/shared/ds/Treemap.tsx")}';
import {text,parseChartDecimal} from LIB_ENTRY;
window.geometry={lineCoordinates,donutArcs,treemapRectangles};
window.draw=(name,data,theme='light')=>render(createElement({LineChart,AreaChart,DonutChart,Treemap}[name],{data,theme,label:text('app.title','ko'),empty:text('ds.tableEmpty','ko')}));
window.fire=fireEvent;window.cleanup=cleanup;window.decimal=parseChartDecimal;
`;

let runtime;
before(async () => {
  runtime = await componentsDom(source);
});
after(() => runtime.close());
test("DS06 좌표 경계와 배치 보존 속성", () => {
  {
    const w = runtime.open();
    const { lineCoordinates, donutArcs, treemapRectangles } = w.geometry;
    const data = (values) =>
      values.map((value, i) => ({ label: `항목${i}`, value: String(value) }));
    const line = lineCoordinates(data([-3, 1, 5]));
    assert.equal(line.kind, "ready");
    assert.deepEqual(
      Array.from(line.points, (p) => [p.x, p.y]),
      [
        [5, 55],
        [50, 30],
        [95, 5],
      ],
    );
    assert.equal(line.minimum, -3);
    assert.equal(line.maximum, 5);
    assert.deepEqual(
      Array.from(lineCoordinates(data([7])).points, (p) => [p.x, p.y]),
      [[50, 30]],
    );
    assert.deepEqual(
      Array.from(lineCoordinates(data([0, 0])).points, (p) => p.y),
      [30, 30],
    );
    assert.equal(lineCoordinates([]).kind, "empty");
    assert.equal(lineCoordinates(data(Array(500).fill(1))).kind, "ready");
    assert.equal(
      lineCoordinates([
        { label: "a", value: "1" },
        { label: "a", value: "2" },
      ]).kind,
      "invalid",
    );
    assert.equal(lineCoordinates([{ label: "", value: "1" }]).kind, "invalid");
    assert.equal(lineCoordinates(data(["999999999999999", "0"])).kind, "ready");
    assert.equal(donutArcs(data([1, 199])).kind, "ready");
    assert.equal(donutArcs(data([1, 200])).kind, "invalid");
    assert.equal(treemapRectangles(data([1, 199])).kind, "ready");
    assert.equal(treemapRectangles(data([1, 200])).kind, "invalid");
    const full = donutArcs(data([7]));
    assert.equal(full.kind, "ready");
    assert.match(full.arcs[0].path, /A25 25 0 1 1 50 55 A25/);
    assert.match(full.arcs[0].path, /A15 15 0 1 0 50 45 A15/);
    const half = donutArcs(data([3, 3]));
    assert.equal(half.arcs[0].end, Math.PI);
    assert.equal(half.arcs[1].start, Math.PI);

    for (const bad of [
      "",
      "NaN",
      "Infinity",
      "1e3",
      " 1",
      "01",
      "1.",
      "9007199254740992",
      "0.00000000000000001",
      "1.2345678901234567",
    ]) {
      assert.equal(lineCoordinates(data([bad, 2])).kind, "invalid", bad);
      assert.equal(donutArcs(data([bad, 2])).kind, "invalid", bad);
      assert.equal(treemapRectangles(data([bad, 2])).kind, "invalid", bad);
    }
    assert.equal(lineCoordinates(data(Array(501).fill(1))).kind, "invalid");
    for (const fn of [donutArcs, treemapRectangles]) {
      assert.equal(fn([]).kind, "empty");
      assert.equal(fn(data([0, 0])).kind, "empty");
      assert.equal(fn(data([-1, 2])).kind, "invalid");
      assert.equal(fn(data(Array(7).fill(1))).kind, "invalid");
    }
    assert.equal(donutArcs(data([1, 1000])).kind, "invalid");
    for (let n = 1; n <= 6; n++)
      for (let seed = 1; seed <= 40; seed++) {
        const values = Array.from(
          { length: n },
          (_, i) => ((seed * 17 + i * 31) % 97) + 10,
        );
        const input = data(values);
        const arcs = donutArcs(input);
        assert.equal(arcs.kind, "ready");
        assert.equal(arcs.arcs[0].start, 0);
        assert.equal(arcs.arcs.at(-1).end, 2 * Math.PI);
        assert.ok(
          Math.abs(
            arcs.arcs.reduce((sum, a) => sum + a.end - a.start, 0) -
              2 * Math.PI,
          ) < 1e-12,
        );
        const rects = treemapRectangles(input);
        assert.equal(rects.kind, "ready");
        assert.equal(rects.rectangles[0].x, 0);
        assert.equal(
          rects.rectangles.at(-1).x + rects.rectangles.at(-1).width,
          100,
        );
        assert.ok(
          Math.abs(
            rects.rectangles.reduce((sum, r) => sum + r.width * r.height, 0) -
              6000,
          ) < 1e-9,
        );
        let edge = 0;
        for (let i = 0; i < n; i++) {
          const r = rects.rectangles[i];
          assert.ok(Math.abs(r.x - edge) < 1e-12);
          assert.equal(r.label, input[i].label);
          assert.equal(r.height, 60);
          assert.ok(r.width > 0);
          assert.ok(
            Math.abs(
              r.width / 100 - values[i] / values.reduce((a, b) => a + b, 0),
            ) < 1e-12,
          );
          edge = r.x + r.width;
        }
      }
    const zero = treemapRectangles(data([0, 2, 0, 3]));
    assert.equal(zero.kind, "ready");
    assert.deepEqual(
      Array.from(zero.rectangles, (r) => r.label),
      ["항목1", "항목3"],
    );
    assert.equal(w.decimal("1.25").kind, "ok");
    assert.equal(w.decimal("bad").kind, "err");
  }
});

test("DS06 접근성, 원문, 키보드와 포인터 표시", async () => {
  {
    const w = runtime.open();
    for (const component of ["LineChart", "AreaChart", "DonutChart", "Treemap"])
      for (const theme of ["light", "dark"]) {
        const view = w.draw(
          component,
          [
            { label: "첫 항목", value: "12.125" },
            { label: "둘째 항목", value: "23.875" },
          ],
          theme,
        );
        assert.equal(
          view.container.querySelector("[role=img]").getAttribute("aria-label"),
          "주식 코파일럿",
        );
        assert.equal(
          view.container.querySelector("[role=img]").getAttribute("viewBox"),
          "0 0 100 60",
        );
        assert.match(
          view.container.querySelector("table").textContent,
          /첫 항목12\.125둘째 항목23\.875/,
        );
        const marks = view.container.querySelectorAll('[tabindex="0"]');
        assert.equal(marks.length, 2);
        w.fire.focus(marks[0]);
        assert.match(
          view.container.querySelector("[role=status]").textContent,
          /첫 항목.*12\.125/,
        );
        w.fire.blur(marks[0]);
        assert.equal(
          view.container.querySelector("[role=status]").textContent,
          "",
        );
        w.fire.pointerEnter(marks[1]);
        assert.match(
          view.container.querySelector("[role=status]").textContent,
          /둘째 항목.*23\.875/,
        );
        w.fire.pointerLeave(marks[1]);
        assert.equal(
          view.container.querySelector("[role=status]").textContent,
          "",
        );
        assert.match(view.container.innerHTML, /chart-1/);
        assert.equal(
          view.container.querySelector("figure").getAttribute("data-theme"),
          theme,
        );
        if (component === "LineChart")
          assert.equal(
            view.container.querySelector("svg>path").getAttribute("d"),
            "M5 55 L95 5",
          );
        if (component === "AreaChart")
          assert.match(
            view.container.querySelector("svg>path").getAttribute("d"),
            /L95 55 L5 55 Z$/,
          );
        if (component === "DonutChart" || component === "Treemap")
          assert.match(marks[1].getAttribute("class"), /fill-chart-2/);

        w.cleanup();
        const empty = w.draw(component, []);
        assert.match(empty.container.textContent, /표시할 항목이 없어요/);
        assert.equal(empty.container.querySelector("svg[role=img]"), null);
        w.cleanup();
        const bad = w.draw(component, [{ label: "a", value: "bad" }]);
        assert.match(bad.container.textContent, /차트 값을 읽을 수 없어요/);
        assert.equal(bad.container.querySelector("svg[role=img]"), null);
        w.cleanup();
      }
  }
});
