import type { ReactElement } from "react";
import type { DockPanel } from "../../lib";
import { useDockSession } from "./controller";
import { dockText } from "./messages";

const commands = [
  ["back", "forward", "up", "down"],
  ["narrower", "wider", "shorter", "taller"],
] as const;
type GeometryAction = (typeof commands)[0 | 1][0 | 1 | 2 | 3];
function useAdjustment(panel: DockPanel): (command: GeometryAction) => void {
  const session = useDockSession();
  return (command: GeometryAction): void => {
    const handle = session?.handle;
    const frame = handle
      ?.getModel()
      .floating?.find((item) => item.tabs.some((tab) => tab.data === panel));
    if (handle === null || handle === undefined || frame === undefined) return;
    const changes = {
      back(): void {
        handle.updateTabset(frame.id, { x: frame.x > 24 ? frame.x - 24 : 0 });
      },
      forward(): void {
        handle.updateTabset(frame.id, { x: frame.x + 24 });
      },
      up(): void {
        handle.updateTabset(frame.id, { y: frame.y > 24 ? frame.y - 24 : 0 });
      },
      down(): void {
        handle.updateTabset(frame.id, { y: frame.y + 24 });
      },
      narrower(): void {
        handle.updateTabset(frame.id, {
          width: frame.width > 240 ? frame.width - 48 : 192,
        });
      },
      wider(): void {
        handle.updateTabset(frame.id, { width: frame.width + 48 });
      },
      shorter(): void {
        handle.updateTabset(frame.id, {
          height: frame.height > 224 ? frame.height - 48 : 176,
        });
      },
      taller(): void {
        handle.updateTabset(frame.id, { height: frame.height + 48 });
      },
    };
    changes[command]();
  };
}
export function FloatingControls({
  panel,
}: {
  readonly panel: DockPanel;
}): ReactElement {
  const adjust = useAdjustment(panel);
  return (
    <details data-dock-geometry="">
      <summary className="px-space-2 py-space-1 cursor-pointer">
        {dockText.geometry}
      </summary>
      {commands.map((row) => (
        <div
          data-dock-options=""
          className="flex flex-wrap gap-space-1 py-space-1"
          key={row[0]}
        >
          {row.map((command) => (
            <button
              type="button"
              key={command}
              className="px-space-2 py-space-1 bg-bg rounded-sm cursor-pointer"
              onClick={(): void => {
                adjust(command);
              }}
            >
              {dockText[command]}
            </button>
          ))}
        </div>
      ))}
    </details>
  );
}
