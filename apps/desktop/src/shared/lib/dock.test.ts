import { expect, it } from "vitest";
import {
  defaultDock,
  restoreDock,
  type DockModel,
  type DockPanel,
} from "./dock";
import {
  groupsOf,
  addStaticPanel,
  removeStaticPanel,
  type DockGroup,
} from "./dock-frames";

const panels = [
  "overview",
  "positions",
  "allocation",
  "briefing",
  "signals",
  "holdings",
  "candidates",
  "economy",
] as const;
const group = (ids: readonly DockPanel[] = []): DockGroup => ({
  type: "tabset" as const,
  id: "main",
  tabs: ids.map((id) => ({ id, data: id })),
});
const panelIds = (model: DockModel): readonly DockPanel[] =>
  groupsOf(model).flatMap((frame) => frame.tabs.map((tab) => tab.data));
const restored = (model: DockModel | undefined): DockModel => {
  expect.assert(model !== undefined);
  return model;
};
const empty = { version: 1, root: group() } as const;
const floating = {
  ...group(["holdings", "signals"]),
  id: "outside",
  selectedId: "signals",
  x: 55,
  y: 48,
  width: 440,
  height: 320,
};

it.each([
  "compact" as const,
  "summary-row" as const,
  "two-columns" as const,
  "three-columns" as const,
])(
  "keeps every widget once and each selected tab on reopening %s",
  (profile) => {
    const model = defaultDock(profile);
    expect(panelIds(model).toSorted()).toEqual(panels.toSorted());
    expect(panelIds(restored(restoreDock(model)))).toEqual(panelIds(model));
    groupsOf(model).forEach((frame) => {
      expect(frame.tabs.map((tab) => tab.id)).toContain(frame.selectedId);
    });
  },
);

it("reopens empty, partially removed and floating-only dashboards", () => {
  expect(panelIds(restored(restoreDock(empty)))).toEqual([]);
  expect(
    panelIds(
      restored(
        restoreDock({ ...empty, root: group(["positions", "overview"]) }),
      ),
    ),
  ).toEqual(["positions", "overview"]);
  const reopened = restored(restoreDock({ ...empty, floating: [floating] }));
  expect(reopened.root).toMatchObject({ type: "tabset", tabs: [] });
  expect(reopened.floating).toMatchObject([floating]);
});

it("rejects duplicate and unknown widgets or versions and restores canonical tab metadata", () => {
  expect(restoreDock({ ...empty, version: 9 })).toBeUndefined();
  expect(
    restoreDock({ ...empty, root: group(["overview", "overview"]) }),
  ).toBeUndefined();
  expect(
    restoreDock({
      ...empty,
      root: { ...group(), tabs: [{ id: "alien", data: "alien" }] },
    }),
  ).toBeUndefined();
  expect(
    restoreDock({ ...empty, root: group(["signals"]), floating: [floating] }),
  ).toBeUndefined();
  const saved = {
    ...group(),
    tabs: [
      {
        id: "positions",
        data: "holdings",
        disableClose: true,
        title: "untrusted",
      },
    ],
  };
  const tab = groupsOf(restored(restoreDock({ ...empty, root: saved })))
    .at(0)
    ?.tabs.at(0);
  expect(tab).toMatchObject({ id: "positions", data: "positions" });
  expect(tab).not.toHaveProperty("disableClose");
  expect(tab).not.toHaveProperty("title", "untrusted");
});

it("restores split ratios, selected tab, tab order and maximized group", () => {
  const left = { ...group(["overview"]), id: "left", size: 37 };
  const right = {
    ...group(["positions", "briefing"]),
    id: "right",
    selectedId: "briefing",
    size: 63,
  };
  const model = restored(
    restoreDock({
      version: 1,
      root: {
        type: "split",
        id: "two",
        orientation: "horizontal",
        children: [left, right],
      },
      maximizedTabsetId: "right",
    }),
  );
  expect(model.root).toMatchObject({ children: [left, right] });
  expect(panelIds(model)).toEqual(["overview", "positions", "briefing"]);
  expect(model.maximizedTabsetId).toBe("right");
});

it("rejects damaged geometry, duplicate frame ids and recursive splits", () => {
  expect(
    restoreDock({ ...empty, floating: [{ ...floating, width: 0 }] }),
  ).toBeUndefined();
  const repeated = {
    type: "split",
    id: "all",
    orientation: "vertical",
    children: [group(["positions"]), group(["overview"])],
  };
  expect(restoreDock({ ...empty, root: repeated })).toBeUndefined();
  type CyclicSplit = { readonly children: readonly CyclicSplit[] };
  const cycle = {
    ...repeated,
    get children(): readonly CyclicSplit[] {
      return [cycle, cycle];
    },
  };
  expect(restoreDock({ ...empty, root: cycle })).toBeUndefined();
});

it("removes a selected floating tab, selects its sibling and re-adds it to the docked group", () => {
  const model = restored(
    restoreDock({
      ...empty,
      root: group(["overview"]),
      floating: [floating],
    }),
  );
  const removed = removeStaticPanel(model, "signals");
  expect(removed.floating).toMatchObject([
    { id: "outside", selectedId: "holdings", tabs: [{ id: "holdings" }] },
  ]);
  const added = addStaticPanel(removed, "signals");
  expect(added.root).toMatchObject({
    selectedId: "signals",
    tabs: [{ id: "overview" }, { id: "signals" }],
  });
  expect(panelIds(restored(restoreDock(added)))).toEqual([
    "overview",
    "signals",
    "holdings",
  ]);
  const withoutFloat = removeStaticPanel(added, "holdings");
  expect(withoutFloat.floating).toEqual([]);
  const blank = removeStaticPanel(
    removeStaticPanel(withoutFloat, "signals"),
    "overview",
  );
  expect(panelIds(restored(restoreDock(blank)))).toEqual([]);
  expect(
    addStaticPanel(restored(restoreDock(blank)), "briefing").root,
  ).toMatchObject({
    selectedId: "briefing",
    tabs: [{ id: "briefing" }],
  });
});

it("keeps the surviving split or floating group and its active tab when a sibling is removed", () => {
  const overview = { ...group(["overview"]), id: "account", size: 40 };
  const reading = {
    ...group(["briefing", "economy"]),
    id: "reading",
    selectedId: "economy",
    size: 60,
  };
  const fixture = restored(
    restoreDock({
      version: 1,
      root: {
        type: "split",
        id: "columns",
        orientation: "horizontal",
        children: [overview, reading],
      },
      floating: [floating],
    }),
  );
  const closed = removeStaticPanel(fixture, "overview");
  expect(closed.root).toMatchObject({ id: "reading", selectedId: "economy" });
  const reopened = addStaticPanel(closed, "overview");
  expect(reopened.root).toMatchObject({
    id: "reading",
    selectedId: "overview",
    tabs: [{ id: "briefing" }, { id: "economy" }, { id: "overview" }],
  });
  expect(restored(restoreDock(reopened)).floating).toMatchObject([floating]);
});
