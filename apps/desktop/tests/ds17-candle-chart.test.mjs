import assert from "node:assert/strict";
import { join } from "node:path";
import test from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";
import { desktopRoot } from "./paths.mjs";

const candleModule = JSON.stringify(
  join(desktopRoot, "src/shared/ds/CandleChart.tsx"),
);
const platformModule = JSON.stringify(
  join(desktopRoot, "src/shared/platform/index.ts"),
);

const harnessSource = `
import { createElement } from "react";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { CandleChart } from DS_ENTRY;
import { candleCoordinates } from ${candleModule};
import { readCanvasTheme, useCanvasChartTheme } from ${platformModule};
import { parseCanvasColor, parseChartDecimal, parseEntityId, text } from LIB_ENTRY;

window.cleanup = cleanup;
window.act = act;
window.fireEvent = fireEvent;
window.candleCoordinates = candleCoordinates;
window.readCanvasTheme = readCanvasTheme;

const dec = (value) => {
  const parsed = parseChartDecimal(value);
  if (parsed.kind !== "ok") throw new Error("decimal: " + value);
  return parsed.value;
};
const id = (value) => {
  const parsed = parseEntityId(value);
  if (parsed.kind !== "ok") throw new Error("id: " + value);
  return parsed.value;
};
const color = (value) => {
  const parsed = parseCanvasColor(value);
  if (parsed.kind !== "ok") throw new Error("color: " + value);
  return parsed.value;
};

window.makeCandles = (rows) =>
  rows.map((row) => ({
    id: id(row.id),
    label: row.label,
    open: dec(row.open),
    high: dec(row.high),
    low: dec(row.low),
    close: dec(row.close),
  }));

window.makePalette = (colors) => ({
  surface: color(colors.surface),
  grid: color(colors.grid),
  axisText: color(colors.axisText),
  rising: color(colors.rising),
  falling: color(colors.falling),
  flat: color(colors.flat),
  focus: color(colors.focus),
});

window.renderCandle = (rows, theme = "light", palette) =>
  render(
    createElement(CandleChart, {
      data: window.makeCandles(rows),
      label: text("app.title", "ko"),
      empty: text("ds.tableEmpty", "ko"),
      theme,
      ...(palette === undefined ? {} : { palette }),
    }),
  );

window.rerenderCandle = (view, rows, theme = "light", palette) =>
  view.rerender(
    createElement(CandleChart, {
      data: window.makeCandles(rows),
      label: text("app.title", "ko"),
      empty: text("ds.tableEmpty", "ko"),
      theme,
      ...(palette === undefined ? {} : { palette }),
    }),
  );

const ThemedProbe = ({ rows, theme }) => {
  const palette = useCanvasChartTheme(theme);
  return createElement(CandleChart, {
    data: window.makeCandles(rows),
    label: text("app.title", "ko"),
    empty: text("ds.tableEmpty", "ko"),
    theme,
    ...(palette === undefined ? {} : { palette }),
  });
};

window.renderWithPlatformTheme = (rows, theme = "light") =>
  render(createElement(ThemedProbe, { rows, theme }));
`;

const sampleRows = [
  {
    id: "d1",
    label: "11월 11일",
    open: "70000",
    high: "72500",
    low: "69500",
    close: "72000",
  },
  {
    id: "d2",
    label: "11월 12일",
    open: "72000",
    high: "72200",
    low: "70100",
    close: "70500",
  },
  {
    id: "d3",
    label: "11월 13일",
    open: "70500",
    high: "71000",
    low: "70000",
    close: "70500",
  },
];

const lightTokens = {
  "--color-surface": "#ffffff",
  "--color-line": "#e3e3de",
  "--color-ink-muted": "#575a60",
  "--color-up": "#d0202d",
  "--color-down": "#1f5bd1",
  "--color-focus-ring": "#0b7258",
};

const darkTokens = {
  "--color-surface": "#16191c",
  "--color-line": "#272c31",
  "--color-ink-muted": "#a3a9af",
  "--color-up": "#ff5d5d",
  "--color-down": "#5e9cff",
  "--color-focus-ring": "#3dc9a3",
};

const installCanvasAndTheme = (w, initialTokens) => {
  const state = { tokens: initialTokens };
  const draws = [];
  w.getComputedStyle = () => ({
    getPropertyValue: (name) => state.tokens[name] ?? "",
  });
  w.HTMLCanvasElement.prototype.getBoundingClientRect = () =>
    new w.DOMRect(0, 0, 300, 150);
  w.HTMLCanvasElement.prototype.getContext = function getContext(kind) {
    if (kind !== "2d") return null;
    const ops = [];
    const ctx = {
      fillStyle: "",
      strokeStyle: "",
      lineWidth: 1,
      clearRect: (...args) => {
        ops.push({ op: "clearRect", args });
      },
      fillRect: (...args) => {
        ops.push({ op: "fillRect", fillStyle: ctx.fillStyle, args });
      },
      beginPath: () => {
        ops.push({ op: "beginPath" });
      },
      moveTo: (...args) => {
        ops.push({ op: "moveTo", args });
      },
      lineTo: (...args) => {
        ops.push({ op: "lineTo", args });
      },
      stroke: () => {
        ops.push({
          op: "stroke",
          strokeStyle: ctx.strokeStyle,
          lineWidth: ctx.lineWidth,
        });
      },
    };
    draws.push(ops);
    return ctx;
  };
  return {
    draws,
    setTokens: (next) => {
      state.tokens = next;
    },
  };
};

test("DS17 candleCoordinates classifies directions and rejects invalid OHLC series", async () => {
  const bundle = await componentsDom(harnessSource);
  try {
    const w = bundle.open();
    assert.equal(w.candleCoordinates([]).kind, "empty");

    const ready = w.candleCoordinates(w.makeCandles(sampleRows));
    assert.equal(ready.kind, "ready");
    assert.equal(ready.bars.length, 3);
    assert.deepEqual(
      Array.from(ready.bars, (bar) => bar.direction),
      ["up", "down", "flat"],
    );
    assert.equal(ready.axes.minimum, "69500");
    assert.equal(ready.axes.maximum, "72500");
    assert.equal(ready.axes.first, "11월 11일");
    assert.equal(ready.axes.last, "11월 13일");

    const invalidCases = [
      [
        {
          id: "bad-high",
          label: "11월 11일",
          open: "100",
          high: "99",
          low: "90",
          close: "95",
        },
      ],
      [
        {
          id: "bad-low",
          label: "11월 11일",
          open: "100",
          high: "110",
          low: "101",
          close: "105",
        },
      ],
      [
        {
          id: "dup",
          label: "11월 11일",
          open: "100",
          high: "110",
          low: "90",
          close: "105",
        },
        {
          id: "dup",
          label: "11월 12일",
          open: "105",
          high: "112",
          low: "100",
          close: "108",
        },
      ],
      [
        {
          id: "blank-label",
          label: "   ",
          open: "100",
          high: "110",
          low: "90",
          close: "105",
        },
      ],
    ];
    for (const rows of invalidCases) {
      assert.equal(w.candleCoordinates(w.makeCandles(rows)).kind, "invalid");
    }
  } finally {
    bundle.close();
  }
});

test("DS17 CandleChart draws on Canvas with CSS theme tokens and redraws on theme changes", async () => {
  const bundle = await componentsDom(harnessSource);
  try {
    const w = bundle.open();
    const { draws, setTokens } = installCanvasAndTheme(w, lightTokens);
    const view = w.renderCandle(sampleRows, "light");
    const figure = view.container.querySelector("figure");
    assert.equal(figure?.dataset.chartTheme, "light");
    assert.ok(view.container.querySelector("canvas"));
    assert.equal(draws.length, 1);
    const lightFills = draws[0]
      .filter((entry) => entry.op === "fillRect")
      .map((entry) => entry.fillStyle);
    assert.deepEqual(lightFills, [
      lightTokens["--color-surface"],
      lightTokens["--color-up"],
      lightTokens["--color-down"],
      lightTokens["--color-ink-muted"],
    ]);

    setTokens(darkTokens);
    w.rerenderCandle(view, sampleRows, "dark");
    assert.equal(figure?.dataset.chartTheme, "dark");
    assert.equal(draws.length, 2);
    const darkFills = draws[1]
      .filter((entry) => entry.op === "fillRect")
      .map((entry) => entry.fillStyle);
    assert.deepEqual(darkFills, [
      darkTokens["--color-surface"],
      darkTokens["--color-up"],
      darkTokens["--color-down"],
      darkTokens["--color-ink-muted"],
    ]);

    setTokens(lightTokens);
    await w.act(async () => {
      w.document.documentElement.dataset.theme = "light";
      await Promise.resolve();
    });
    assert.ok(draws.length >= 3);
    const mutatedFills = draws
      .at(-1)
      .filter((entry) => entry.op === "fillRect")
      .map((entry) => entry.fillStyle);
    assert.equal(mutatedFills[0], lightTokens["--color-surface"]);
  } finally {
    bundle.close();
  }
});

test("DS17 platform theme wrapper passes palette props and keyboard navigation exposes OHLC details", async () => {
  const bundle = await componentsDom(harnessSource);
  try {
    const w = bundle.open();
    const { draws } = installCanvasAndTheme(w, darkTokens);
    const view = w.renderWithPlatformTheme(sampleRows, "dark");
    assert.ok(draws.length >= 1);
    const lastFills = draws
      .at(-1)
      .filter((entry) => entry.op === "fillRect")
      .map((entry) => entry.fillStyle);
    assert.equal(lastFills[0], darkTokens["--color-surface"]);

    const explore = view.container.querySelector("button");
    assert.ok(explore);
    w.fireEvent.focus(explore);
    const status = view.container.querySelector('[role="status"]');
    assert.ok(status?.textContent.includes("11월 11일"));
    assert.ok(status?.textContent.includes("70,000"));
    assert.ok(status?.textContent.includes("72,500"));
    assert.ok(status?.textContent.includes("69,500"));
    assert.ok(status?.textContent.includes("72,000"));

    w.fireEvent.keyDown(explore, { key: "ArrowRight" });
    assert.ok(status?.textContent.includes("11월 12일"));
    assert.ok(status?.textContent.includes("70,500"));
    const focusStroke = draws
      .at(-1)
      .some(
        (entry) =>
          entry.op === "stroke" &&
          entry.strokeStyle === darkTokens["--color-focus-ring"],
      );
    assert.equal(focusStroke, true);

    const tableText = view.container.querySelector("table")?.textContent ?? "";
    assert.ok(tableText.includes("69,500"));
    assert.ok(tableText.includes("72,500"));

    w.rerenderCandle(view, [], "light");
    assert.equal(view.container.querySelector("canvas"), null);
    assert.ok(view.container.textContent.includes("표시할 항목이 없어요"));

    w.rerenderCandle(
      view,
      [
        {
          id: "bad",
          label: "11월 11일",
          open: "100",
          high: "90",
          low: "95",
          close: "95",
        },
      ],
      "light",
    );
    assert.equal(view.container.querySelector("canvas"), null);
    assert.ok(view.container.textContent.includes("차트 값을 읽을 수 없어요"));
  } finally {
    bundle.close();
  }
});

test("DS17 uses screen pixels for wicks and keeps candle bodies within their time slots", async () => {
  const bundle = await componentsDom(harnessSource);
  try {
    const w = bundle.open();
    const { draws } = installCanvasAndTheme(w, lightTokens);
    let width = 1920;
    const height = 220;
    w.HTMLCanvasElement.prototype.getBoundingClientRect = () =>
      new w.DOMRect(0, 0, width, height);
    w.Element.prototype.getAnimations = () => [];
    const resizeHandlers = new WeakMap();
    w.ResizeObserver = class {
      constructor(notify) {
        this.notify = notify;
      }
      observe(element) {
        resizeHandlers.set(element, this.notify);
      }
      disconnect() {}
    };
    const queries = new Map();
    w.matchMedia = (name) => {
      const media = new w.EventTarget();
      queries.set(name, media);
      return media;
    };
    const series = (count) =>
      Array.from({ length: count }, (_, i) => ({
        ...sampleRows[i % sampleRows.length],
        id: `bar-${i}`,
      }));
    w.devicePixelRatio = 1;
    const view = w.renderCandle(series(60));
    const canvas = view.container.querySelector("canvas");
    const resize = () => w.act(() => resizeHandlers.get(canvas)());
    const assertPlot = (count) => {
      const dpr = w.devicePixelRatio;
      const ops = draws.at(-1);
      assert.equal(canvas.width, Math.round(width * dpr));
      assert.equal(canvas.height, Math.round(height * dpr));
      const bodies = ops.filter((op) => op.op === "fillRect").slice(1);
      const wicks = ops.filter((op) => op.op === "stroke").slice(3);
      assert.equal(bodies.length, count);
      assert.equal(wicks.length, count);
      for (const [i, body] of bodies.entries()) {
        const [left, , size] = body.args;
        assert.ok(size >= 1 && size <= 16 * dpr);
        assert.ok(wicks[i].lineWidth <= dpr);
        const next = bodies[i + 1];
        if (next !== undefined) {
          const gap = next.args[0] - left - size;
          assert.ok(
            gap >= (Math.floor((0.9 * canvas.width) / count) >= 2 ? 1 : 0),
          );
        }
      }
    };
    assertPlot(60);
    width = 720;
    resize();
    assertPlot(60);
    for (const [oldRatio, newRatio] of [
      [1, 2],
      [2, 1],
    ]) {
      w.devicePixelRatio = newRatio;
      w.act(() => {
        queries
          .get(`(resolution: ${oldRatio}dppx)`)
          .dispatchEvent(new w.Event("change"));
      });
      assertPlot(60);
      w.rerenderCandle(view, series(500));
      assertPlot(500);
      width = 1920;
      resize();
      assertPlot(500);
      width = 720;
      w.rerenderCandle(view, series(60));
    }
    w.rerenderCandle(view, series(1));
    assertPlot(1);
    w.cleanup();
  } finally {
    bundle.close();
  }
});
