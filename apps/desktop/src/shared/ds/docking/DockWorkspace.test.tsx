import { expect, it, vi } from "vitest";
import { render, act } from "@testing-library/react";
import { builtinEnvironments } from "vitest/runtime";
import {
  Workspace,
  type WorkspaceImperativeHandle,
} from "react-resizable-panels";
import { dockTab, restoreDock, type DockPanel } from "../../lib";

type Commands = Readonly<WorkspaceImperativeHandle<DockPanel>>;

const checkMoves = (handle: Commands): void => {
  expect(
    handle.addTab(dockTab("briefing"), { tabsetId: "main", select: true }),
  ).toBe(true);
  expect(
    handle.addTab(dockTab("briefing"), { tabsetId: "main", select: true }),
  ).toBe(false);
  handle.floatTab("briefing", { x: 20, y: 25, width: 420, height: 300 });
  const outside = handle.getModel().floating?.at(0);
  expect.assert(outside !== undefined);
  expect(outside.tabs.map((tab) => tab.id)).toEqual(["briefing"]);
  handle.moveTab("positions", { tabsetId: outside.id, region: "center" });
  handle.selectTab("briefing");
  const saved = restoreDock(handle.getModel());
  expect.assert(saved !== undefined);
  expect(saved.floating).toMatchObject([
    {
      selectedId: "briefing",
      tabs: [{ id: "briefing" }, { id: "positions" }],
    },
  ]);
  expect(saved.root).toMatchObject({ tabs: [{ id: "overview" }] });
  expect(handle.setModel(saved)).toMatchObject({ root: saved.root });
  handle.moveTabset(outside.id, { tabsetId: "main", region: "center" });
  handle.selectTab("positions");
  handle.closeTab("briefing");
  const result = restoreDock(handle.getModel());
  expect(result?.root).toMatchObject({
    selectedId: "positions",
    tabs: [{ id: "overview" }, { id: "positions" }],
  });
  expect(result?.floating ?? []).toEqual([]);
};

it("groups, floats, selects and re-docks widgets with the workspace commands used by the dashboard", async ({
  onTestFinished,
}) => {
  const sandbox = {};
  const environment = await builtinEnvironments.jsdom.setup(sandbox, {});
  expect.assert("document" in sandbox);
  expect.assert("Node" in sandbox);
  const testGlobals = vi
    .stubGlobal("window", sandbox)
    .stubGlobal("document", sandbox.document)
    .stubGlobal("Node", sandbox.Node);
  onTestFinished(() => environment.teardown(sandbox));
  const capture = testGlobals.fn<(handle: Commands | null) => void>();
  const widget: { readonly unmount: () => void } = render(
    <Workspace<DockPanel>
      defaultModel={{
        version: 1,
        root: {
          type: "tabset",
          id: "main",
          tabs: [dockTab("overview"), dockTab("positions")],
        },
      }}
      renderTabContent={(tab) => tab.data}
      workspaceRef={(handle) => {
        capture(handle);
      }}
    />,
  );
  onTestFinished(widget.unmount);
  act(() => {
    const handle = capture.mock.calls.at(0)?.at(0);
    expect.assert(handle !== undefined && handle !== null);
    checkMoves(handle);
  });
});
