import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { ESLint } from "eslint";
import { componentsDom } from "./ds-components-dom.mjs";
import { dsConfigs } from "../eslint/ds-config.mjs";
import { desktopRoot } from "./paths.mjs";

const require = createRequire(join(desktopRoot, "package.json"));
const { compile } = require("tailwindcss");
const { parse } = require("postcss");

let fixture;
before(async () => {
  fixture = await componentsDom(String.raw`
import { createElement as h } from "react";
import { render, cleanup, act, fireEvent } from "@testing-library/react";
import {
  Badge,
  Button,
  Card,
  Checkbox,
  Dialog,
  Disclosure,
  DonutChart,
  LineChart,
  Money,
  Percent,
  PriceChange,
  Quantity,
  SegmentedControl,
  StatusBar,
  Table,
  Text,
  TextField,
  Treemap,
} from DS_ENTRY;
import { ScrollArea } from ${JSON.stringify(join(desktopRoot, "src/shared/ds/ScrollArea.tsx"))};
import {
  emptyInputValue,
  parseChartDecimal,
  parseEntityId,
  parseInputValue,
  parseMoney,
  parsePercentDisplay,
  parseQuantity,
  text,
} from LIB_ENTRY;

const label = text("app.title", "ko");
const description = text("ds.description", "ko");
const money = parseMoney("987654321098", "KRW").value;
const negativeMoney = parseMoney("-100", "KRW").value;
const rate = parsePercentDisplay("12.34").value;
const qty = parseQuantity("123456.789").value;
const idA = parseEntityId("a").value;
const idB = parseEntityId("b").value;
const chartData = [
  { id: idA, label, value: parseChartDecimal("24500000000").value },
  { id: idB, label: description, value: parseChartDecimal("18600000000").value },
];
const optionA = parseInputValue("a").value;
const optionB = parseInputValue("b").value;
const action = { label: description, onPress: () => {} };

const show = async (node) => {
  cleanup();
  let view;
  await act(async () => {
    view = render(node);
    await Promise.resolve();
  });
  return view;
};
window.cleanup = cleanup;
window.fire = fireEvent;
window.showScroll = (direction) =>
  show(h(ScrollArea, { direction }, h(Text, null, description)));
window.showNumbers = () =>
  show(
    h(
      "div",
      null,
      h(Money, { value: money }),
      h(Money, { value: negativeMoney, direction: "up" }),
      h(Percent, { value: rate }),
      h(Quantity, { value: qty }),
      h(PriceChange, { amount: money, rate, direction: "up" }),
    ),
  );
window.showCard = () =>
  show(h(Card, { title: label, actions: action }, h(Money, { value: money })));
window.showTable = () =>
  show(
    h(
      Table.Root,
      { caption: label },
      h(
        Table.Body,
        null,
        h(Table.Row, null, h(Table.RowHeader, null, label), h(Table.Cell, { align: "end" }, h(Money, { value: money }))),
      ),
    ),
  );
window.showChart = (kind) => {
  const Chart = kind === "line" ? LineChart : kind === "donut" ? DonutChart : Treemap;
  return show(h(Chart, { data: chartData, label, empty: description, theme: "light" }));
};
window.showDialog = () =>
  show(
    h(
      Dialog,
      { open: "open", title: label, onClose: () => {}, actions: { cancel: action, confirm: action } },
      h(Text, null, description),
    ),
  );
window.showButton = (size) => show(h(Button, { size }, label));
window.showSecretField = () =>
  show(h(TextField, { label, type: "secret", defaultValue: emptyInputValue }));
window.showCheckbox = () => show(h(Checkbox, { label, checked: "checked" }));
window.showBadge = () => show(h(Badge, null, description));
window.showSegmented = () =>
  show(
    h(SegmentedControl, {
      label,
      options: [
        { value: optionA, label },
        { value: optionB, label: description },
      ],
    }),
  );
window.showStatusBar = () =>
  show(h(StatusBar, { tone: "warning", action }, description));
window.showDisclosure = () =>
  show(h(Disclosure, { title: label }, h(Text, null, description)));
`);
});
after(() => fixture?.close());

test("DS23 ScrollArea declares directional viewport, overlay scrollbar suppression, and focus ring", async () => {
  const w = fixture.open();
  for (const [direction, viewportClass] of [
    ["horizontal", "scroll-viewport-x"],
    ["vertical", "scroll-viewport-y"],
    ["both", "scroll-viewport-both"],
  ]) {
    await w.showScroll(direction);
    const viewport = w.document.querySelector("[data-scroll-viewport]");
    assert.ok(viewport, direction);
    assert.ok(viewport.classList.contains(viewportClass), direction);
    assert.ok(viewport.classList.contains("base-ui-disable-scrollbar"));
    assert.ok(viewport.classList.contains("focus-visible:outline-solid"));
    assert.ok(viewport.classList.contains("focus-visible:outline-focus"));
    assert.ok(viewport.classList.contains("focus-visible:outline-focus-ring"));
    assert.ok(
      viewport.classList.contains("focus-visible:scroll-outline-inset"),
    );
    if (direction === "horizontal")
      assert.ok(viewport.classList.contains("pb-space-2"));
    assert.equal(viewport.parentElement.style.position, "relative");
    const content = viewport.firstElementChild;
    assert.ok(content, direction);
    assert.equal(content.getAttribute("role"), "presentation", direction);
    if (direction === "vertical")
      assert.ok(content.classList.contains("scroll-content-y"));
  }
  const tokensCss = readFileSync(
    join(desktopRoot, "src/shared/ds/tokens.css"),
    "utf8",
  );
  const themeCss = readFileSync(
    join(desktopRoot, "src/shared/ds/theme.css"),
    "utf8",
  ).replace(/^@import[^;]*;$/gmu, "");
  const source = `${readFileSync(join(desktopRoot, "node_modules/tailwindcss/theme.css"), "utf8")}\n${tokensCss}\n${themeCss}\n@tailwind utilities;`;
  const compiled = (await compile(source)).build([
    "scroll-viewport-x",
    "scroll-viewport-y",
    "scroll-viewport-both",
    "scroll-content-y",
    "scroll-outline-inset",
  ]);
  const declarations = {};
  parse(compiled).walkRules((rule) => {
    rule.walkDecls((decl) => {
      declarations[`${rule.selector}:${decl.prop}`] = decl.important
        ? `${decl.value} !important`
        : decl.value;
    });
  });
  assert.equal(declarations[".scroll-viewport-x:position"], "relative");
  assert.equal(
    declarations[".scroll-viewport-x:overflow-x"],
    "auto !important",
  );
  assert.equal(
    declarations[".scroll-viewport-x:overflow-y"],
    "hidden !important",
  );
  assert.equal(declarations[".scroll-viewport-y:position"], "relative");
  assert.equal(
    declarations[".scroll-viewport-y:overflow-x"],
    "hidden !important",
  );
  assert.equal(
    declarations[".scroll-viewport-y:overflow-y"],
    "auto !important",
  );
  assert.equal(declarations[".scroll-viewport-both:position"], "relative");
  assert.equal(
    declarations[".scroll-viewport-both:overflow-x"],
    "auto !important",
  );
  assert.equal(
    declarations[".scroll-viewport-both:overflow-y"],
    "auto !important",
  );
  assert.equal(declarations[".scroll-content-y:min-width"], "0 !important");
  assert.equal(declarations[".scroll-content-y:overflow-x"], "clip");
  assert.equal(
    declarations[".scroll-outline-inset:outline-offset"],
    "calc(var(--border-width-focus) * -1)",
  );
});

test("DS23 numbers stay inline single-line while Card, Table, and Donut/Treemap provide horizontal ScrollArea", async () => {
  const w = fixture.open();
  await w.showNumbers();
  assert.equal(w.document.querySelector("[data-scroll-viewport]"), null);
  const spans = w.document.querySelectorAll(".tabular-nums.whitespace-nowrap");
  assert.equal(spans.length, 5);
  for (const span of spans) assert.ok(span.classList.contains("w-max"));

  for (const run of [() => w.showCard(), () => w.showTable()]) {
    await run();
    const viewports = w.document.querySelectorAll("[data-scroll-viewport]");
    assert.equal(viewports.length, 1);
    assert.ok(viewports[0].classList.contains("scroll-viewport-x"));
  }

  for (const kind of ["donut", "treemap"]) {
    await w.showChart(kind);
    const viewports = [
      ...w.document.querySelectorAll("[data-scroll-viewport]"),
    ];
    assert.equal(viewports.length, 1, kind);
    assert.ok(viewports[0].classList.contains("scroll-viewport-x"), kind);
    assert.ok(viewports[0].querySelector("table") !== null, kind);
  }

  await w.showChart("line");
  const lineViewports = [
    ...w.document.querySelectorAll("[data-scroll-viewport]"),
  ];
  assert.equal(lineViewports.length, 2);
  for (const vp of lineViewports)
    assert.ok(vp.classList.contains("scroll-viewport-x"));
  assert.ok(
    lineViewports[1].querySelector(".\\@content-medium\\:justify-end") !== null,
  );
  const explore = w.document.querySelector("button");
  w.fire.focus(explore);
  const focusedViewports = [
    ...w.document.querySelectorAll("[data-scroll-viewport]"),
  ];
  assert.equal(focusedViewports.length, 3);
  for (const vp of focusedViewports)
    assert.ok(vp.classList.contains("scroll-viewport-x"));
  const statusRegion = w.document.querySelector('[role="status"]');
  const initialStatusText = statusRegion.textContent;
  assert.ok(initialStatusText.length > 0);
  w.fire.blur(explore);
  assert.equal(statusRegion.textContent, "");
  w.fire.focus(statusRegion);
  assert.equal(statusRegion.textContent, initialStatusText);
  w.fire.blur(statusRegion);
  assert.equal(statusRegion.textContent, "");
  const srTable = w.document.querySelector("table.sr-only");
  assert.ok(srTable);
  assert.equal(srTable.closest("[data-scroll-viewport]"), null);
});

test("DS23 buttons, secret TextField, Dialog, and five resolved components apply approved layouts", async () => {
  const w = fixture.open();
  for (const size of ["control-height", "control-height-sm"]) {
    await w.showButton(size);
    const button = w.document.querySelector("button");
    assert.ok(button.classList.contains(`min-h-${size}`));
    assert.equal(button.classList.contains(`h-${size}`), false);
    assert.ok(button.classList.contains("status-sentence"));
  }

  await w.showSecretField();
  assert.ok(w.document.querySelector(".flex-wrap"));
  assert.ok(w.document.querySelector(".wrap-slot"));

  await w.showDialog();
  const popup = w.document.querySelector('[aria-modal="true"]');
  assert.ok(popup.classList.contains("max-h-full"));
  assert.ok(
    popup.querySelector("header").classList.contains("min-h-toolbar-height"),
  );
  assert.ok(popup.querySelector("footer").classList.contains("flex-wrap"));
  const dialogViewport = popup.querySelector("[data-scroll-viewport]");
  assert.ok(dialogViewport);
  assert.ok(dialogViewport.classList.contains("scroll-viewport-both"));

  await w.showCheckbox();
  const checkbox = w.document.querySelector('button[role="checkbox"]');
  assert.ok(checkbox.classList.contains("size-icon-sm"));
  const checkSvg = checkbox.querySelector("svg");
  assert.ok(checkSvg.classList.contains("size-space-3"));
  assert.ok(
    checkSvg.parentElement.classList.contains("items-center") &&
      checkSvg.parentElement.classList.contains("justify-center"),
  );

  await w.showBadge();
  const badge = w.document.querySelector("[data-badge]");
  assert.equal(badge.classList.contains("whitespace-nowrap"), false);
  assert.ok(badge.classList.contains("status-sentence"));

  await w.showSegmented();
  assert.ok(
    w.document
      .querySelector("[role='group']")
      ?.classList.contains("flex-wrap") ??
      w.document.querySelector(".flex-wrap"),
  );
  for (const toggle of w.document.querySelectorAll("button")) {
    assert.ok(toggle.classList.contains("min-h-control-height-sm"));
    assert.equal(toggle.classList.contains("h-control-height-sm"), false);
  }

  await w.showStatusBar();
  const region = w.document.querySelector('[role="status"]');
  const bar = region.parentElement.parentElement;
  assert.ok(bar.classList.contains("px-space-4"));
  assert.ok(bar.classList.contains("flex-wrap"));
  assert.ok(region.parentElement.classList.contains("wrap-slot"));
  assert.ok(region.parentElement.querySelector("svg") !== null);

  await w.showDisclosure();
  const trigger = w.document.querySelector("button");
  assert.ok(trigger.classList.contains("min-h-toolbar-height"));
  assert.equal(trigger.classList.contains("h-toolbar-height"), false);
  assert.ok(trigger.classList.contains("text-left"));
  assert.ok(trigger.querySelector(".status-sentence"));
});

test("DS23 new scroll and layout utilities remain restricted to their owning files", async () => {
  const engine = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: dsConfigs(),
  });
  for (const [file, cls] of [
    ["src/shared/ds/Badge.tsx", "scroll-viewport-x"],
    ["src/shared/ds/Badge.tsx", "scroll-viewport-y"],
    ["src/shared/ds/Badge.tsx", "scroll-viewport-both"],
    ["src/shared/ds/Badge.tsx", "scroll-track-x"],
    ["src/shared/ds/Badge.tsx", "scroll-track-y"],
    ["src/shared/ds/Badge.tsx", "scroll-content-y"],
    ["src/shared/ds/Text.tsx", "scroll-content-y"],
    ["src/shared/ds/Badge.tsx", "wrap-slot"],
    ["src/shared/ds/Card.tsx", "scroll-viewport-x"],
    ["src/shared/ds/Text.tsx", "scroll-viewport-x"],
  ]) {
    const [result] = await engine.lintText(
      `const p = <div className="${cls}" />;`,
      {
        filePath: file,
      },
    );
    assert.ok(
      result.messages.some(
        (m) => m.ruleId === "better-tailwindcss/no-restricted-classes",
      ),
      `${file}: ${cls}`,
    );
  }
});
