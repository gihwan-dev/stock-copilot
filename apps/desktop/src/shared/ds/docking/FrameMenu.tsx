import { useState, type ReactElement } from "react";
import type { TabDropRegion } from "react-resizable-panels";
import { useFrame, type CurrentFrame } from "./useFrame";
import {
  parseInputValue,
  type DockModel,
  type DockPanel,
  type InputValue,
} from "../../lib";
import { Select } from "../Select";
import { groupsOf, type DockGroup } from "./controller";
import { DockPopover, useCloseDockMenu } from "./DockPopover";
import { FloatingControls } from "./FloatingControls";
import { dockText, dockTitle, dockLayoutLabel } from "./messages";

type Option = {
  readonly label: typeof dockText.frame;
  readonly value: InputValue;
};
type MoveScope = "tab" | "group";
export const activeDockPanel = (
  group: Readonly<DockGroup>,
): DockPanel | undefined =>
  (group.tabs.find((tab) => tab.id === group.selectedId) ?? group.tabs.at(0))
    ?.data;
const regions = ["left", "right", "top", "bottom", "center"] as const;
const frameOption = (entry: Readonly<DockGroup>): Option[] => {
  const id = parseInputValue(entry.id);
  const title = activeDockPanel(entry);
  return id.kind === "ok"
    ? [
        {
          value: id.value,
          label: title === undefined ? dockText.emptyTarget : dockTitle[title],
        },
      ]
    : [];
};
function Scope(props: {
  readonly scope: MoveScope;
  readonly select: (scope: MoveScope) => void;
}): ReactElement {
  return (
    <fieldset data-dock-scope="" className="flex items-center gap-space-1">
      <legend>{dockText.scope}</legend>
      {(["tab", "group"] as const).map((choice) => (
        <button
          type="button"
          key={choice}
          aria-pressed={choice === props.scope}
          className={
            choice === props.scope
              ? "px-space-2 py-space-1 rounded-sm text-primary bg-primary-soft cursor-pointer"
              : "px-space-2 py-space-1 rounded-sm text-ink-muted bg-bg cursor-pointer"
          }
          onClick={(): void => {
            props.select(choice);
          }}
        >
          {dockText[choice]}
        </button>
      ))}
    </fieldset>
  );
}
const getTargets = (
  model: Readonly<DockModel>,
  group: DockGroup,
  scope: MoveScope,
): readonly Option[] => {
  const others = groupsOf(model)
    .filter((entry) => entry.id !== group.id)
    .flatMap(frameOption);
  const sameDockedGroup =
    model.floating?.some((item) => item.id === group.id) !== true;
  return scope === "tab" && group.tabs.length > 1 && sameDockedGroup
    ? [...others, ...frameOption(group)]
    : others;
};
const canDetach = (
  frame: CurrentFrame,
  scope: MoveScope,
): "available" | "unavailable" => {
  const { group, model, location } = frame;
  const floating = location === "floating";
  const allowed =
    scope === "group"
      ? !floating && group.id !== model.root.id
      : !floating || group.tabs.length > 1;
  return allowed ? "available" : "unavailable";
};
function PinFrame({
  panel,
  target,
}: {
  readonly panel: DockPanel;
  readonly target: InputValue;
}): ReactElement | null {
  const frame = useFrame(panel);
  const close = useCloseDockMenu();
  if (frame?.location !== "floating") return null;
  const pin = (): void => {
    const docked = groupsOf({ version: 1, root: frame.model.root });
    const anchor = docked.find((group) => group.id === target) ?? docked.at(0);
    if (anchor === undefined) return;
    close();
    frame.handle.moveTabset(frame.group.id, {
      tabsetId: anchor.id,
      region: anchor.tabs.length === 0 ? "center" : "right",
    });
    frame.handle.selectTab(panel);
    frame.focus(panel);
  };
  return (
    <button
      type="button"
      data-dock-pin=""
      className="px-space-2 py-space-1 text-left text-primary cursor-pointer hover:bg-surface-sunken focus-visible:outline-focus"
      onClick={pin}
    >
      {dockText.dock}
    </button>
  );
}
function MoveTo(props: {
  readonly panel: DockPanel;
  readonly scope: MoveScope;
}): ReactElement | null {
  const frame = useFrame(props.panel);
  const close = useCloseDockMenu();
  const [selected, setSelected] = useState<InputValue>();
  if (frame === undefined) return null;
  const { focus, handle, model, group } = frame;
  const options = getTargets(model, group, props.scope);
  const target =
    options.find((item) => item.value === selected)?.value ??
    options.at(0)?.value;
  if (target === undefined) return null;
  const targetFloats =
    model.floating?.some((item) => item.id === target) === true;
  const move = (region: TabDropRegion): void => {
    close();
    const at = { region, tabsetId: target };
    if (props.scope === "group") handle.moveTabset(group.id, at);
    else handle.moveTab(props.panel, at);
    handle.selectTab(props.panel);
    focus(props.panel);
  };
  return (
    <>
      <Select
        label={dockText.frame}
        options={options}
        value={target}
        onChange={setSelected}
      />
      <PinFrame panel={props.panel} target={target} />
      <div data-dock-moves="" className="flex flex-wrap gap-space-1">
        {regions.map((region) => (
          <button
            type="button"
            key={region}
            data-dock-region={region}
            className="px-space-2 py-space-1 rounded-sm bg-bg text-body-sm cursor-pointer hover:bg-surface-sunken focus-visible:outline-focus disabled:opacity-disabled"
            disabled={region === "center" ? target === group.id : targetFloats}
            onClick={(): void => {
              move(region);
            }}
          >
            {dockText[region]}
          </button>
        ))}
      </div>
    </>
  );
}
function Placement({
  panel,
}: {
  readonly panel: DockPanel;
}): ReactElement | null {
  const frame = useFrame(panel);
  const close = useCloseDockMenu();
  const [scope, setScope] = useState<MoveScope>("tab");
  if (frame === undefined) return null;
  const { focus, handle, group, location } = frame;
  const canFloat = canDetach(frame, scope);
  const detach = (): void => {
    close();
    if (scope === "group") handle.floatTabset(group.id);
    else handle.floatTab(panel);
    focus(panel);
  };
  return (
    <>
      {group.tabs.length > 1 && <Scope scope={scope} select={setScope} />}
      <MoveTo panel={panel} scope={scope} />
      {canFloat === "available" && (
        <button
          type="button"
          data-dock-action=""
          className="px-space-2 py-space-1 text-left text-primary cursor-pointer hover:bg-surface-sunken focus-visible:outline-focus"
          onClick={detach}
        >
          {dockText.float}
        </button>
      )}
      {location === "floating" && <FloatingControls panel={panel} />}
      <button
        type="button"
        data-dock-footer=""
        className="px-space-2 pt-space-2 text-left border-t-hairline border-solid border-line cursor-pointer hover:text-primary focus-visible:outline-focus"
        onClick={(): void => {
          close();
          handle.closeTab(panel);
          focus();
        }}
      >
        {dockText.remove}
      </button>
    </>
  );
}
export function FrameMenu({
  panel,
}: {
  readonly panel: DockPanel;
}): ReactElement {
  return (
    <DockPopover kind="layout" label={dockLayoutLabel[panel]}>
      <Placement panel={panel} />
    </DockPopover>
  );
}
