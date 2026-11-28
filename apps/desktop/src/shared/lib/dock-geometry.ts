import type { FloatingTabsetRect } from "react-resizable-panels";
import type { DockArea } from "./brand";
import type { DockModel } from "./dock-panels";

type Rect = Readonly<FloatingTabsetRect>;

const boundedRect = (rect: Rect, area: DockArea): Rect => {
  const width = Math.min(rect.width, area.width);
  const height = Math.min(rect.height, area.height);
  return {
    width,
    height,
    x: Math.max(0, Math.min(rect.x, area.width - width)),
    y: Math.max(0, Math.min(rect.y, area.height - height)),
  };
};

const comparison = (a: Rect, b: Rect): "equal" | "different" =>
  a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height
    ? "equal"
    : "different";

export function visibleDockModel(
  model: DockModel,
  preferred: DockModel,
  area: DockArea | undefined,
): DockModel | undefined {
  if (area === undefined) return undefined;
  const shown = model.floating?.map((frame) => {
    const source = preferred.floating?.find((other) => other.id === frame.id);
    const rect = boundedRect(source ?? frame, area);
    return comparison(frame, rect) === "equal" ? frame : { ...frame, ...rect };
  });
  return shown?.some((frame, index) => frame !== model.floating?.at(index)) ===
    true
    ? { ...model, floating: shown }
    : undefined;
}

const preserveUnchanged = (next: Rect, old: Rect, area: DockArea): Rect => {
  const prior = boundedRect(old, area);
  const bounded = boundedRect(next, area);
  return {
    x: next.x === prior.x && next.width === prior.width ? old.x : bounded.x,
    y: next.y === prior.y && next.height === prior.height ? old.y : bounded.y,
    width: next.width === prior.width ? old.width : bounded.width,
    height: next.height === prior.height ? old.height : bounded.height,
  };
};

export function preferredDockAfterEdit(
  model: DockModel,
  previous: DockModel | undefined,
  area: DockArea | undefined,
): DockModel {
  if (area === undefined || model.floating === undefined) return model;
  return {
    ...model,
    floating: model.floating.map((frame) => {
      const old = previous?.floating?.find((other) => other.id === frame.id);
      const bounds =
        old === undefined
          ? boundedRect(frame, area)
          : preserveUnchanged(frame, old, area);
      return { ...frame, ...bounds };
    }),
  };
}
