import type { FloatingTabsetRect } from "react-resizable-panels";
import { expect, it } from "vitest";
import { restoreDock, measureDockArea } from "./brand";
import { preferredDockAfterEdit, visibleDockModel } from "./dock-geometry";
import type { DockModel } from "./dock-panels";

const frame = { x: 750, y: 500, width: 600, height: 850 };
const modelWith = (geometry: Readonly<FloatingTabsetRect>): DockModel => {
  const model = restoreDock({
    version: 1,
    root: {
      type: "tabset",
      id: "root",
      tabs: [{ id: "signals", data: "signals" }],
    },
    floating: [
      {
        type: "tabset",
        id: "float",
        tabs: [{ id: "briefing", data: "briefing" }],
        ...geometry,
      },
    ],
  });
  expect.assert(model !== undefined);
  return model;
};
const preferred = modelWith(frame);
const wide = measureDockArea(1400, 1400);
const small = measureDockArea(720, 600);

it("fits the whole frame and restores the preferred size when the workspace grows", () => {
  const visible = visibleDockModel(preferred, preferred, small);
  expect.assert(visible !== undefined);
  expect(visible.floating?.at(0)).toMatchObject({
    x: 120,
    y: 0,
    width: 600,
    height: 600,
  });
  expect(visibleDockModel(visible, preferred, small)).toBeUndefined();
  expect(
    visibleDockModel(visible, preferred, wide)?.floating?.at(0),
  ).toMatchObject(frame);
  expect(
    preferredDockAfterEdit(visible, preferred, small).floating?.at(0),
  ).toMatchObject(frame);
});
it("uses the bounded edit on one axis and preserves the untouched axis", () => {
  const resized = modelWith({ x: 120, y: 0, width: 700, height: 600 });
  const edited = preferredDockAfterEdit(resized, preferred, small);
  expect(edited.floating?.at(0)).toMatchObject({
    x: 20,
    width: 700,
    y: 500,
    height: 850,
  });
});
it.each([undefined, preferred])(
  "saves bounded frame coordinates on a new or existing float",
  (previous) => {
    const moved = modelWith({ x: 1400, y: 1400, width: 600, height: 850 });
    expect(
      preferredDockAfterEdit(moved, previous, wide).floating?.at(0),
    ).toMatchObject({ x: 800, y: 550, width: 600, height: 850 });
  },
);
it("ignores hidden or unmeasurable workspaces", () => {
  expect(measureDockArea(0, 100)).toBeUndefined();
  expect(measureDockArea(100, NaN)).toBeUndefined();
  expect(visibleDockModel(preferred, preferred, undefined)).toBeUndefined();
});
